use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    DealerContinuation, EndReason, Honba, Payment, ProgressError, RiichiRules, RiichiSticks,
    RiichiVariant, RoundNumber, Seat, TableProgress, WinEvaluation, Wind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredWinner {
    seat: Seat,
    evaluation: WinEvaluation,
}

impl ScoredWinner {
    #[must_use]
    pub const fn new(seat: Seat, evaluation: WinEvaluation) -> Self {
        Self { seat, evaluation }
    }

    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn evaluation(&self) -> &WinEvaluation {
        &self.evaluation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandOutcome {
    Tsumo {
        winner: ScoredWinner,
    },
    Ron {
        from: Seat,
        winners: Box<[ScoredWinner]>,
    },
    ExhaustiveDraw {
        tenpai: Box<[Seat]>,
        nagashi_winners: Box<[Seat]>,
    },
    AbortiveDraw {
        reason: EndReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandResult {
    reason: EndReason,
    progress: TableProgress,
    points_before: Box<[i32]>,
    point_deltas: Box<[i32]>,
    points_after: Box<[i32]>,
    winners: Box<[ScoredWinner]>,
    from: Option<Seat>,
    tenpai: Box<[Seat]>,
    nagashi_winners: Box<[Seat]>,
    awarded_riichi_sticks: u32,
    dealer_continues: bool,
    next_progress: TableProgress,
}

impl HandResult {
    #[must_use]
    pub const fn reason(&self) -> EndReason {
        self.reason
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub fn points_before(&self) -> &[i32] {
        &self.points_before
    }

    #[must_use]
    pub fn point_deltas(&self) -> &[i32] {
        &self.point_deltas
    }

    #[must_use]
    pub fn points_after(&self) -> &[i32] {
        &self.points_after
    }

    #[must_use]
    pub fn winners(&self) -> &[ScoredWinner] {
        &self.winners
    }

    #[must_use]
    pub const fn from(&self) -> Option<Seat> {
        self.from
    }

    #[must_use]
    pub fn tenpai(&self) -> &[Seat] {
        &self.tenpai
    }

    #[must_use]
    pub fn nagashi_winners(&self) -> &[Seat] {
        &self.nagashi_winners
    }

    #[must_use]
    pub const fn awarded_riichi_sticks(&self) -> u32 {
        self.awarded_riichi_sticks
    }

    #[must_use]
    pub const fn dealer_continues(&self) -> bool {
        self.dealer_continues
    }

    #[must_use]
    pub const fn next_progress(&self) -> TableProgress {
        self.next_progress
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HandSettlement;

impl HandSettlement {
    pub fn settle(
        self,
        rules: &RiichiRules,
        progress: TableProgress,
        points: impl IntoIterator<Item = i32>,
        outcome: HandOutcome,
    ) -> Result<HandResult, SettlementError> {
        if rules.variant != progress.variant() {
            return Err(SettlementError::VariantMismatch);
        }
        let points_before: Vec<_> = points.into_iter().collect();
        let seat_count = usize::from(rules.variant.seat_count().value());
        if points_before.len() != seat_count {
            return Err(SettlementError::PointCount {
                expected: seat_count,
                actual: points_before.len(),
            });
        }
        validate_outcome(rules, &outcome)?;

        let mut deltas = vec![0_i32; seat_count];
        let resolved = resolve_outcome(rules, progress, &mut deltas, outcome)?;
        let points_after = points_before
            .iter()
            .zip(&deltas)
            .map(|(points, delta)| {
                points
                    .checked_add(*delta)
                    .ok_or(SettlementError::PointOverflow)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let riichi_sticks = if resolved.awarded_riichi_sticks > 0 {
            RiichiSticks::ZERO
        } else {
            progress.riichi_sticks()
        };
        let next_progress = next_progress(
            progress,
            resolved.dealer_continues,
            matches!(resolved.reason, EndReason::Tsumo | EndReason::Ron),
            riichi_sticks,
        )?;

        Ok(HandResult {
            reason: resolved.reason,
            progress,
            points_before: points_before.into_boxed_slice(),
            point_deltas: deltas.into_boxed_slice(),
            points_after: points_after.into_boxed_slice(),
            winners: resolved.winners,
            from: resolved.from,
            tenpai: resolved.tenpai,
            nagashi_winners: resolved.nagashi_winners,
            awarded_riichi_sticks: resolved.awarded_riichi_sticks,
            dealer_continues: resolved.dealer_continues,
            next_progress,
        })
    }
}

struct ResolvedOutcome {
    reason: EndReason,
    winners: Box<[ScoredWinner]>,
    from: Option<Seat>,
    tenpai: Box<[Seat]>,
    nagashi_winners: Box<[Seat]>,
    awarded_riichi_sticks: u32,
    dealer_continues: bool,
}

fn resolve_outcome(
    rules: &RiichiRules,
    progress: TableProgress,
    deltas: &mut [i32],
    outcome: HandOutcome,
) -> Result<ResolvedOutcome, SettlementError> {
    match outcome {
        HandOutcome::Tsumo { winner } => {
            apply_tsumo(deltas, progress, &winner)?;
            let awarded = award_riichi_sticks(deltas, progress, winner.seat)?;
            let dealer_continues = winner.seat == progress.dealer();
            Ok(ResolvedOutcome {
                reason: EndReason::Tsumo,
                winners: vec![winner].into_boxed_slice(),
                from: None,
                tenpai: Box::new([]),
                nagashi_winners: Box::new([]),
                awarded_riichi_sticks: awarded,
                dealer_continues,
            })
        }
        HandOutcome::Ron { from, winners } => {
            for winner in &winners {
                apply_ron(deltas, progress, from, winner)?;
            }
            let awarded = award_riichi_sticks(deltas, progress, winners[0].seat)?;
            let dealer_continues = winners
                .iter()
                .any(|winner| winner.seat == progress.dealer());
            Ok(ResolvedOutcome {
                reason: EndReason::Ron,
                winners,
                from: Some(from),
                tenpai: Box::new([]),
                nagashi_winners: Box::new([]),
                awarded_riichi_sticks: awarded,
                dealer_continues,
            })
        }
        HandOutcome::ExhaustiveDraw {
            tenpai,
            nagashi_winners,
        } => {
            let (awarded, dealer_continues) = if nagashi_winners.is_empty() {
                apply_noten(rules, deltas, &tenpai)?;
                (
                    0,
                    matches!(
                        rules.match_rules.dealer_continuation,
                        DealerContinuation::WinOrTenpai
                    ) && tenpai.contains(&progress.dealer()),
                )
            } else {
                apply_nagashi(deltas, progress, &nagashi_winners)?;
                (
                    award_riichi_sticks(deltas, progress, nagashi_winners[0])?,
                    nagashi_winners.contains(&progress.dealer()),
                )
            };
            Ok(ResolvedOutcome {
                reason: EndReason::ExhaustiveDraw,
                winners: Box::new([]),
                from: None,
                tenpai,
                nagashi_winners,
                awarded_riichi_sticks: awarded,
                dealer_continues,
            })
        }
        HandOutcome::AbortiveDraw { reason } => Ok(ResolvedOutcome {
            reason,
            winners: Box::new([]),
            from: None,
            tenpai: Box::new([]),
            nagashi_winners: Box::new([]),
            awarded_riichi_sticks: 0,
            dealer_continues: true,
        }),
    }
}

fn validate_outcome(rules: &RiichiRules, outcome: &HandOutcome) -> Result<(), SettlementError> {
    let variant = rules.variant;
    let valid_seat = |seat: Seat| seat.index() < variant.seat_count().value();
    match outcome {
        HandOutcome::Tsumo { winner } => {
            if !valid_seat(winner.seat) {
                return Err(SettlementError::InvalidSeat);
            }
            if !matches!(winner.evaluation.payment(), Payment::Tsumo { .. }) {
                return Err(SettlementError::PaymentKindMismatch);
            }
        }
        HandOutcome::Ron { from, winners } => {
            if !valid_seat(*from) || winners.is_empty() {
                return Err(SettlementError::InvalidWinnerSet);
            }
            let mut seen = vec![false; usize::from(variant.seat_count().value())];
            for winner in winners {
                if !valid_seat(winner.seat)
                    || winner.seat == *from
                    || std::mem::replace(&mut seen[usize::from(winner.seat.index())], true)
                {
                    return Err(SettlementError::InvalidWinnerSet);
                }
                if !matches!(winner.evaluation.payment(), Payment::Ron { .. }) {
                    return Err(SettlementError::PaymentKindMismatch);
                }
            }
        }
        HandOutcome::ExhaustiveDraw {
            tenpai,
            nagashi_winners,
        } => {
            validate_unique_seats(variant, tenpai)?;
            validate_unique_seats(variant, nagashi_winners)?;
            if !nagashi_winners.is_empty() && !tenpai.is_empty() {
                return Err(SettlementError::IncompatibleDrawResults);
            }
            if !nagashi_winners.is_empty() && !rules.scoring.nagashi_mangan {
                return Err(SettlementError::NagashiManganDisabled);
            }
        }
        HandOutcome::AbortiveDraw { reason } => {
            if !matches!(
                reason,
                EndReason::NineTerminals
                    | EndReason::FourWinds
                    | EndReason::FourKans
                    | EndReason::FourRiichi
            ) {
                return Err(SettlementError::InvalidAbortiveReason);
            }
        }
    }
    Ok(())
}

fn validate_unique_seats(variant: RiichiVariant, seats: &[Seat]) -> Result<(), SettlementError> {
    let mut seen = vec![false; usize::from(variant.seat_count().value())];
    for seat in seats {
        if seat.index() >= variant.seat_count().value()
            || std::mem::replace(&mut seen[usize::from(seat.index())], true)
        {
            return Err(SettlementError::InvalidWinnerSet);
        }
    }
    Ok(())
}

fn apply_ron(
    deltas: &mut [i32],
    progress: TableProgress,
    from: Seat,
    winner: &ScoredWinner,
) -> Result<(), SettlementError> {
    let Payment::Ron { points } = winner.evaluation.payment() else {
        return Err(SettlementError::PaymentKindMismatch);
    };
    let honba = progress
        .honba()
        .value()
        .checked_mul(300)
        .ok_or(SettlementError::PointOverflow)?;
    transfer(
        deltas,
        from,
        winner.seat,
        points
            .checked_add(honba)
            .ok_or(SettlementError::PointOverflow)?,
    )
}

fn apply_tsumo(
    deltas: &mut [i32],
    progress: TableProgress,
    winner: &ScoredWinner,
) -> Result<(), SettlementError> {
    let Payment::Tsumo {
        dealer_payment,
        other_payment,
    } = winner.evaluation.payment()
    else {
        return Err(SettlementError::PaymentKindMismatch);
    };
    let honba = progress
        .honba()
        .value()
        .checked_mul(100)
        .ok_or(SettlementError::PointOverflow)?;
    for index in 0..deltas.len() {
        let from = seat_at(progress.variant(), index);
        if from == winner.seat {
            continue;
        }
        let base = if from == progress.dealer() {
            dealer_payment
        } else {
            other_payment
        };
        transfer(
            deltas,
            from,
            winner.seat,
            base.checked_add(honba)
                .ok_or(SettlementError::PointOverflow)?,
        )?;
    }
    Ok(())
}

fn apply_nagashi(
    deltas: &mut [i32],
    progress: TableProgress,
    winners: &[Seat],
) -> Result<(), SettlementError> {
    let honba = progress
        .honba()
        .value()
        .checked_mul(100)
        .ok_or(SettlementError::PointOverflow)?;
    for winner in winners {
        let winner_is_dealer = *winner == progress.dealer();
        for index in 0..deltas.len() {
            let from = seat_at(progress.variant(), index);
            if from == *winner {
                continue;
            }
            let base: u32 = if winner_is_dealer || from == progress.dealer() {
                4_000
            } else {
                2_000
            };
            transfer(
                deltas,
                from,
                *winner,
                base.checked_add(honba)
                    .ok_or(SettlementError::PointOverflow)?,
            )?;
        }
    }
    Ok(())
}

fn apply_noten(
    rules: &RiichiRules,
    deltas: &mut [i32],
    tenpai: &[Seat],
) -> Result<(), SettlementError> {
    if tenpai.is_empty() || tenpai.len() == deltas.len() || rules.settlement.noten_payment == 0 {
        return Ok(());
    }
    let noten_count = deltas.len() - tenpai.len();
    let total = rules.settlement.noten_payment;
    let tenpai_count = u32::try_from(tenpai.len()).expect("tenpai count");
    let noten_count = u32::try_from(noten_count).expect("noten count");
    if !is_divisible(total, tenpai_count) || !is_divisible(total, noten_count) {
        return Err(SettlementError::IndivisibleNotenPayment);
    }
    let gain = i32::try_from(total / tenpai_count).map_err(|_| SettlementError::PointOverflow)?;
    let loss = i32::try_from(total / noten_count).map_err(|_| SettlementError::PointOverflow)?;
    for (index, delta) in deltas.iter_mut().enumerate() {
        let seat = seat_at(rules.variant, index);
        *delta = if tenpai.contains(&seat) { gain } else { -loss };
    }
    Ok(())
}

#[allow(clippy::manual_is_multiple_of)]
const fn is_divisible(value: u32, divisor: u32) -> bool {
    value % divisor == 0
}

fn award_riichi_sticks(
    deltas: &mut [i32],
    progress: TableProgress,
    winner: Seat,
) -> Result<u32, SettlementError> {
    let sticks = progress.riichi_sticks().value();
    if sticks == 0 {
        return Ok(0);
    }
    add_delta(
        deltas,
        winner,
        sticks
            .checked_mul(1_000)
            .ok_or(SettlementError::PointOverflow)?,
    )?;
    Ok(sticks)
}

fn transfer(deltas: &mut [i32], from: Seat, to: Seat, points: u32) -> Result<(), SettlementError> {
    let points = i32::try_from(points).map_err(|_| SettlementError::PointOverflow)?;
    let from_delta = deltas[usize::from(from.index())]
        .checked_sub(points)
        .ok_or(SettlementError::PointOverflow)?;
    let to_delta = deltas[usize::from(to.index())]
        .checked_add(points)
        .ok_or(SettlementError::PointOverflow)?;
    deltas[usize::from(from.index())] = from_delta;
    deltas[usize::from(to.index())] = to_delta;
    Ok(())
}

fn add_delta(deltas: &mut [i32], seat: Seat, points: u32) -> Result<(), SettlementError> {
    let points = i32::try_from(points).map_err(|_| SettlementError::PointOverflow)?;
    deltas[usize::from(seat.index())] = deltas[usize::from(seat.index())]
        .checked_add(points)
        .ok_or(SettlementError::PointOverflow)?;
    Ok(())
}

fn next_progress(
    progress: TableProgress,
    dealer_continues: bool,
    was_win: bool,
    riichi_sticks: RiichiSticks,
) -> Result<TableProgress, SettlementError> {
    if dealer_continues {
        return TableProgress::new(
            progress.variant(),
            progress.round_wind(),
            progress.round_number(),
            progress.dealer(),
            progress
                .honba()
                .checked_increment()
                .map_err(SettlementError::Progress)?,
            riichi_sticks,
        )
        .map_err(SettlementError::Progress);
    }

    let variant = progress.variant();
    let seat_count = variant.seat_count().value();
    let dealer =
        Seat::new(variant, (progress.dealer().index() + 1) % seat_count).expect("rotated dealer");
    let (round_wind, round_number) = if progress.round_number().value() < seat_count {
        (
            progress.round_wind(),
            RoundNumber::new(variant, progress.round_number().value() + 1)
                .map_err(SettlementError::Progress)?,
        )
    } else {
        (
            next_wind(progress.round_wind()),
            RoundNumber::new(variant, 1).map_err(SettlementError::Progress)?,
        )
    };
    let honba = if was_win {
        Honba::ZERO
    } else {
        progress
            .honba()
            .checked_increment()
            .map_err(SettlementError::Progress)?
    };
    TableProgress::new(
        variant,
        round_wind,
        round_number,
        dealer,
        honba,
        riichi_sticks,
    )
    .map_err(SettlementError::Progress)
}

fn seat_at(variant: RiichiVariant, index: usize) -> Seat {
    Seat::new(
        variant,
        u8::try_from(index).expect("mahjong seat index fits u8"),
    )
    .expect("bounded seat")
}

const fn next_wind(wind: Wind) -> Wind {
    match wind {
        Wind::East => Wind::South,
        Wind::South => Wind::West,
        Wind::West => Wind::North,
        Wind::North => Wind::East,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettlementError {
    VariantMismatch,
    PointCount { expected: usize, actual: usize },
    InvalidSeat,
    InvalidWinnerSet,
    PaymentKindMismatch,
    IncompatibleDrawResults,
    NagashiManganDisabled,
    InvalidAbortiveReason,
    IndivisibleNotenPayment,
    PointOverflow,
    Progress(ProgressError),
}

impl Display for SettlementError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariantMismatch => {
                formatter.write_str("rules and table progress variants do not match")
            }
            Self::PointCount { expected, actual } => {
                write!(formatter, "expected {expected} point entries, got {actual}")
            }
            Self::InvalidSeat => formatter.write_str("outcome contains an invalid seat"),
            Self::InvalidWinnerSet => formatter.write_str("outcome contains invalid winners"),
            Self::PaymentKindMismatch => {
                formatter.write_str("win evaluation payment does not match outcome")
            }
            Self::IncompatibleDrawResults => {
                formatter.write_str("nagashi mangan and noten settlement cannot be combined")
            }
            Self::NagashiManganDisabled => {
                formatter.write_str("nagashi mangan is disabled by active rules")
            }
            Self::InvalidAbortiveReason => {
                formatter.write_str("abortive outcome requires an abortive end reason")
            }
            Self::IndivisibleNotenPayment => {
                formatter.write_str("noten payment cannot be divided evenly")
            }
            Self::PointOverflow => formatter.write_str("point calculation overflow"),
            Self::Progress(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for SettlementError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Progress(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BonusHan, DealerContinuation, EndReason, HandOutcome, HandSettlement, HandShape, Honba,
        Limit, Payment, RiichiRules, RiichiSticks, RiichiVariant, RoundNumber, ScoredWinner, Seat,
        TableProgress, WaitKind, WinEvaluation, Wind,
    };

    fn progress(
        variant: RiichiVariant,
        dealer: Seat,
        honba: u32,
        riichi_sticks: u32,
    ) -> TableProgress {
        TableProgress::new(
            variant,
            Wind::East,
            RoundNumber::new(variant, 1).expect("round"),
            dealer,
            Honba::new(honba),
            RiichiSticks::new(riichi_sticks),
        )
        .expect("progress")
    }

    fn evaluation(payment: Payment) -> WinEvaluation {
        WinEvaluation::new(
            HandShape::Standard,
            WaitKind::TwoSided,
            Vec::<crate::YakuValue>::new(),
            BonusHan::default(),
            1,
            30,
            0,
            240,
            Limit::None,
            payment,
        )
    }

    #[test]
    fn multiple_ron_charges_each_winner_and_awards_sticks_to_first() {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let from = Seat::new(RiichiVariant::Yonma, 1).expect("from");
        let first = Seat::new(RiichiVariant::Yonma, 2).expect("first");
        let second = Seat::new(RiichiVariant::Yonma, 3).expect("second");

        let result = HandSettlement
            .settle(
                &rules,
                progress(RiichiVariant::Yonma, dealer, 1, 2),
                [25_000; 4],
                HandOutcome::Ron {
                    from,
                    winners: vec![
                        ScoredWinner::new(first, evaluation(Payment::Ron { points: 3_900 })),
                        ScoredWinner::new(second, evaluation(Payment::Ron { points: 8_000 })),
                    ]
                    .into_boxed_slice(),
                },
            )
            .expect("settlement");

        assert_eq!(result.point_deltas(), [0, -12_500, 6_200, 8_300]);
        assert_eq!(result.awarded_riichi_sticks(), 2);
        assert!(!result.dealer_continues());
        assert_eq!(result.next_progress().round_number().value(), 2);
        assert_eq!(result.next_progress().honba().value(), 0);
        assert_eq!(result.next_progress().riichi_sticks().value(), 0);
    }

    #[test]
    fn sanma_tsumo_uses_only_two_actual_payers() {
        let rules = RiichiRules::standard(RiichiVariant::Sanma);
        let dealer = Seat::new(RiichiVariant::Sanma, 0).expect("dealer");
        let winner = Seat::new(RiichiVariant::Sanma, 1).expect("winner");

        let result = HandSettlement
            .settle(
                &rules,
                progress(RiichiVariant::Sanma, dealer, 1, 0),
                [25_000; 3],
                HandOutcome::Tsumo {
                    winner: ScoredWinner::new(
                        winner,
                        evaluation(Payment::Tsumo {
                            dealer_payment: 2_000,
                            other_payment: 1_000,
                        }),
                    ),
                },
            )
            .expect("settlement");

        assert_eq!(result.point_deltas(), [-2_100, 3_200, -1_100]);
        assert_eq!(result.next_progress().dealer(), winner);
        assert_eq!(result.next_progress().honba().value(), 0);
    }

    #[test]
    fn exhaustive_draw_splits_noten_and_applies_continuation_rule() {
        let mut rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let other = Seat::new(RiichiVariant::Yonma, 2).expect("other");
        let outcome = || HandOutcome::ExhaustiveDraw {
            tenpai: vec![dealer, other].into_boxed_slice(),
            nagashi_winners: Box::new([]),
        };

        let continued = HandSettlement
            .settle(
                &rules,
                progress(RiichiVariant::Yonma, dealer, 0, 1),
                [25_000; 4],
                outcome(),
            )
            .expect("continued");
        assert_eq!(continued.point_deltas(), [1_500, -1_500, 1_500, -1_500]);
        assert!(continued.dealer_continues());
        assert_eq!(continued.next_progress().honba().value(), 1);
        assert_eq!(continued.next_progress().riichi_sticks().value(), 1);

        rules.match_rules.dealer_continuation = DealerContinuation::WinOnly;
        let advanced = HandSettlement
            .settle(
                &rules,
                progress(RiichiVariant::Yonma, dealer, 0, 1),
                [25_000; 4],
                outcome(),
            )
            .expect("advanced");
        assert!(!advanced.dealer_continues());
        assert_eq!(advanced.next_progress().round_number().value(), 2);
        assert_eq!(advanced.next_progress().honba().value(), 1);
    }

    #[test]
    fn disabled_nagashi_mangan_is_rejected_by_settlement_boundary() {
        let mut rules = RiichiRules::default();
        rules.scoring.nagashi_mangan = false;
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");

        let error = HandSettlement
            .settle(
                &rules,
                progress(RiichiVariant::Yonma, dealer, 0, 0),
                [25_000; 4],
                HandOutcome::ExhaustiveDraw {
                    tenpai: Box::new([]),
                    nagashi_winners: vec![dealer].into_boxed_slice(),
                },
            )
            .expect_err("disabled nagashi");

        assert_eq!(error, crate::SettlementError::NagashiManganDisabled);
    }

    #[test]
    fn abortive_draw_repeats_dealer_without_moving_points() {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 1).expect("dealer");
        let current = progress(RiichiVariant::Yonma, dealer, 2, 1);

        let result = HandSettlement
            .settle(
                &rules,
                current,
                [25_000; 4],
                HandOutcome::AbortiveDraw {
                    reason: EndReason::FourKans,
                },
            )
            .expect("abortive draw");

        assert_eq!(result.point_deltas(), [0; 4]);
        assert_eq!(result.next_progress().dealer(), dealer);
        assert_eq!(result.next_progress().honba().value(), 3);
        assert_eq!(result.next_progress().riichi_sticks().value(), 1);
    }
}
