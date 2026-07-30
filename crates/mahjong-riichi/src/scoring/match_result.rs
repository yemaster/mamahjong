use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    EndReason, HandResult, MatchLength, PlacementUma, RiichiRules, RiichiVariant, Seat,
    TableProgress, ValidationErrors, Wind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchEndReason {
    ScheduledEnd,
    Tobi,
    AgariYame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchPlacement {
    seat: Seat,
    rank: u8,
    points: i32,
    uma_tenths: i32,
    oka_tenths: i32,
    score_tenths: i32,
}

impl MatchPlacement {
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn rank(&self) -> u8 {
        self.rank
    }

    #[must_use]
    pub const fn points(&self) -> i32 {
        self.points
    }

    #[must_use]
    pub const fn uma_tenths(&self) -> i32 {
        self.uma_tenths
    }

    #[must_use]
    pub const fn oka_tenths(&self) -> i32 {
        self.oka_tenths
    }

    #[must_use]
    pub const fn score_tenths(&self) -> i32 {
        self.score_tenths
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchResult {
    end_reason: MatchEndReason,
    starting_dealer: Seat,
    hand_count: u32,
    final_points: Box<[i32]>,
    placements: Box<[MatchPlacement]>,
    unclaimed_riichi_sticks_awarded: u32,
}

impl MatchResult {
    #[must_use]
    pub const fn end_reason(&self) -> MatchEndReason {
        self.end_reason
    }

    #[must_use]
    pub const fn starting_dealer(&self) -> Seat {
        self.starting_dealer
    }

    #[must_use]
    pub const fn hand_count(&self) -> u32 {
        self.hand_count
    }

    #[must_use]
    pub fn final_points(&self) -> &[i32] {
        &self.final_points
    }

    #[must_use]
    pub fn placements(&self) -> &[MatchPlacement] {
        &self.placements
    }

    #[must_use]
    pub const fn unclaimed_riichi_sticks_awarded(&self) -> u32 {
        self.unclaimed_riichi_sticks_awarded
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiichiMatch {
    rules: RiichiRules,
    starting_dealer: Seat,
    progress: TableProgress,
    points: Box<[i32]>,
    hands: Vec<HandResult>,
    result: Option<MatchResult>,
}

impl RiichiMatch {
    pub fn start(rules: RiichiRules, starting_dealer: Seat) -> Result<Self, MatchError> {
        rules.validate().map_err(MatchError::InvalidRules)?;
        if starting_dealer.index() >= rules.variant.seat_count().value() {
            return Err(MatchError::InvalidStartingDealer);
        }
        let initial_points = i32::try_from(rules.match_rules.initial_points)
            .map_err(|_| MatchError::ScoreOverflow)?;
        let seat_count = usize::from(rules.variant.seat_count().value());
        let progress = TableProgress::east_one(rules.variant, starting_dealer)
            .map_err(|_| MatchError::InvalidStartingDealer)?;
        Ok(Self {
            rules,
            starting_dealer,
            progress,
            points: vec![initial_points; seat_count].into_boxed_slice(),
            hands: Vec::new(),
            result: None,
        })
    }

    #[must_use]
    pub const fn rules(&self) -> &RiichiRules {
        &self.rules
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub fn points(&self) -> &[i32] {
        &self.points
    }

    #[must_use]
    pub fn hands(&self) -> &[HandResult] {
        &self.hands
    }

    #[must_use]
    pub const fn result(&self) -> Option<&MatchResult> {
        self.result.as_ref()
    }

    pub fn apply_hand(&mut self, hand: HandResult) -> Result<Option<&MatchResult>, MatchError> {
        if self.result.is_some() {
            return Err(MatchError::AlreadyFinished);
        }
        if hand.progress() != self.progress {
            return Err(MatchError::ProgressMismatch);
        }
        if hand.points_before() != self.points.as_ref() {
            return Err(MatchError::PointsMismatch);
        }

        let end_reason = self.end_reason_after(&hand);
        let hand_count =
            u32::try_from(self.hands.len() + 1).map_err(|_| MatchError::HandCountOverflow)?;
        let final_result = end_reason
            .map(|reason| {
                build_match_result(
                    &self.rules,
                    self.starting_dealer,
                    reason,
                    hand_count,
                    hand.points_after(),
                    hand.next_progress().riichi_sticks().value(),
                )
            })
            .transpose()?;

        self.points = hand.points_after().to_vec().into_boxed_slice();
        self.progress = hand.next_progress();
        self.hands.push(hand);
        self.result = final_result;
        Ok(self.result.as_ref())
    }

    fn end_reason_after(&self, hand: &HandResult) -> Option<MatchEndReason> {
        if self.rules.match_rules.tobi && hand.points_after().iter().any(|points| *points < 0) {
            return Some(MatchEndReason::Tobi);
        }
        if !is_last_scheduled_hand(&self.rules, self.progress) {
            return None;
        }
        if self.rules.match_rules.agari_yame
            && hand.dealer_continues()
            && matches!(hand.reason(), EndReason::Tsumo | EndReason::Ron)
            && hand
                .winners()
                .iter()
                .any(|winner| winner.seat() == self.progress.dealer())
            && ranking_order(
                hand.points_after(),
                self.starting_dealer,
                self.rules.variant,
            )[0] == self.progress.dealer()
        {
            return Some(MatchEndReason::AgariYame);
        }
        (!hand.dealer_continues()).then_some(MatchEndReason::ScheduledEnd)
    }
}

fn is_last_scheduled_hand(rules: &RiichiRules, progress: TableProgress) -> bool {
    let final_wind = match rules.match_rules.length {
        MatchLength::EastOnly => Wind::East,
        MatchLength::Hanchan => Wind::South,
    };
    progress.round_wind() == final_wind
        && progress.round_number().value() == rules.variant.seat_count().value()
}

fn build_match_result(
    rules: &RiichiRules,
    starting_dealer: Seat,
    end_reason: MatchEndReason,
    hand_count: u32,
    points: &[i32],
    unclaimed_riichi_sticks: u32,
) -> Result<MatchResult, MatchError> {
    let mut final_points = points.to_vec();
    if unclaimed_riichi_sticks > 0 {
        let leader = ranking_order(&final_points, starting_dealer, rules.variant)[0];
        let award = i32::try_from(
            unclaimed_riichi_sticks
                .checked_mul(1_000)
                .ok_or(MatchError::ScoreOverflow)?,
        )
        .map_err(|_| MatchError::ScoreOverflow)?;
        final_points[usize::from(leader.index())] = final_points[usize::from(leader.index())]
            .checked_add(award)
            .ok_or(MatchError::ScoreOverflow)?;
    }
    let order = ranking_order(&final_points, starting_dealer, rules.variant);
    let uma = placement_uma(rules, &final_points, &order)?;
    let oka_total = (i64::from(rules.match_rules.return_points)
        - i64::from(rules.match_rules.initial_points))
        * i64::from(rules.variant.seat_count().value());
    let oka_tenths = i32::try_from(oka_total / 100).map_err(|_| MatchError::ScoreOverflow)?;
    let return_points =
        i32::try_from(rules.match_rules.return_points).map_err(|_| MatchError::ScoreOverflow)?;

    let mut placements = Vec::with_capacity(order.len());
    for (rank_index, seat) in order.into_iter().enumerate() {
        let rank = u8::try_from(rank_index + 1).expect("mahjong placement fits u8");
        let points = final_points[usize::from(seat.index())];
        let point_tenths = points
            .checked_sub(return_points)
            .ok_or(MatchError::ScoreOverflow)?
            / 100;
        let uma_tenths = uma[rank_index]
            .checked_mul(10)
            .ok_or(MatchError::ScoreOverflow)?;
        let seat_oka = if rank == 1 { oka_tenths } else { 0 };
        let score_tenths = point_tenths
            .checked_add(uma_tenths)
            .and_then(|score| score.checked_add(seat_oka))
            .ok_or(MatchError::ScoreOverflow)?;
        placements.push(MatchPlacement {
            seat,
            rank,
            points,
            uma_tenths,
            oka_tenths: seat_oka,
            score_tenths,
        });
    }

    Ok(MatchResult {
        end_reason,
        starting_dealer,
        hand_count,
        final_points: final_points.into_boxed_slice(),
        placements: placements.into_boxed_slice(),
        unclaimed_riichi_sticks_awarded: unclaimed_riichi_sticks,
    })
}

fn ranking_order(points: &[i32], starting_dealer: Seat, variant: RiichiVariant) -> Vec<Seat> {
    let seat_count = variant.seat_count().value();
    let mut seats: Vec<_> = (0..seat_count)
        .map(|index| Seat::new(variant, index).expect("bounded seat"))
        .collect();
    seats.sort_unstable_by(|left, right| {
        match points[usize::from(right.index())].cmp(&points[usize::from(left.index())]) {
            Ordering::Equal => distance_from(starting_dealer, *left, seat_count)
                .cmp(&distance_from(starting_dealer, *right, seat_count)),
            ordering => ordering,
        }
    });
    seats
}

const fn distance_from(start: Seat, seat: Seat, seat_count: u8) -> u8 {
    (seat.index() + seat_count - start.index()) % seat_count
}

fn placement_uma(
    rules: &RiichiRules,
    points: &[i32],
    order: &[Seat],
) -> Result<Vec<i32>, MatchError> {
    match &rules.settlement.uma {
        PlacementUma::Fixed { values } => {
            Ok(values.iter().map(|value| i32::from(*value)).collect())
        }
        PlacementUma::JpmlA => {
            let return_points = i32::try_from(rules.match_rules.return_points)
                .map_err(|_| MatchError::ScoreOverflow)?;
            let floating = order
                .iter()
                .filter(|seat| points[usize::from(seat.index())] > return_points)
                .count();
            Ok(match floating {
                0 => vec![0, 0, 0, 0],
                1 => vec![12, -1, -3, -8],
                2 => vec![8, 4, -4, -8],
                3 => vec![8, 3, 1, -12],
                _ => return Err(MatchError::InvalidJpmlAState),
            })
        }
    }
}

#[derive(Debug)]
pub enum MatchError {
    InvalidRules(ValidationErrors),
    InvalidStartingDealer,
    AlreadyFinished,
    ProgressMismatch,
    PointsMismatch,
    HandCountOverflow,
    ScoreOverflow,
    InvalidJpmlAState,
}

impl Display for MatchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRules(errors) => Display::fmt(errors, formatter),
            Self::InvalidStartingDealer => formatter.write_str("starting dealer is invalid"),
            Self::AlreadyFinished => formatter.write_str("match is already finished"),
            Self::ProgressMismatch => {
                formatter.write_str("hand progress does not match active match")
            }
            Self::PointsMismatch => formatter.write_str("hand points do not match active match"),
            Self::HandCountOverflow => formatter.write_str("hand count overflow"),
            Self::ScoreOverflow => formatter.write_str("match score calculation overflow"),
            Self::InvalidJpmlAState => formatter.write_str("invalid JPML A floating-score state"),
        }
    }
}

impl Error for MatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRules(errors) => Some(errors),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BonusHan, HandOutcome, HandSettlement, HandShape, Honba, Limit, MatchEndReason,
        MatchLength, Payment, RiichiMatch, RiichiPreset, RiichiRules, RiichiSticks, RiichiVariant,
        RoundNumber, ScoredWinner, Seat, TableProgress, WaitKind, WinEvaluation, Wind,
    };

    use super::{placement_uma, ranking_order};

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

    fn set_hand(
        game: &mut RiichiMatch,
        wind: Wind,
        number: u8,
        dealer: Seat,
        points: &[i32],
        riichi_sticks: u32,
    ) {
        game.progress = TableProgress::new(
            game.rules.variant,
            wind,
            RoundNumber::new(game.rules.variant, number).expect("round"),
            dealer,
            Honba::ZERO,
            RiichiSticks::new(riichi_sticks),
        )
        .expect("progress");
        game.points = points.to_vec().into_boxed_slice();
    }

    #[test]
    fn scheduled_end_keeps_hand_history_and_calculates_oka_uma() {
        let rules = RiichiRules::default();
        let starting_dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let mut game = RiichiMatch::start(rules.clone(), starting_dealer).expect("match");
        set_hand(&mut game, Wind::South, 4, starting_dealer, &[25_000; 4], 0);
        let winner = Seat::new(RiichiVariant::Yonma, 1).expect("winner");
        let from = Seat::new(RiichiVariant::Yonma, 2).expect("from");
        let hand = HandSettlement
            .settle(
                &rules,
                game.progress(),
                game.points().iter().copied(),
                HandOutcome::Ron {
                    from,
                    winners: vec![ScoredWinner::new(
                        winner,
                        evaluation(Payment::Ron { points: 7_700 }),
                    )]
                    .into_boxed_slice(),
                },
            )
            .expect("hand");

        let result = game
            .apply_hand(hand)
            .expect("apply")
            .expect("finished match");

        assert_eq!(result.end_reason(), MatchEndReason::ScheduledEnd);
        assert_eq!(result.hand_count(), 1);
        assert_eq!(result.final_points(), [25_000, 32_700, 17_300, 25_000]);
        assert_eq!(
            result
                .placements()
                .iter()
                .map(|placement| placement.seat())
                .collect::<Vec<_>>(),
            [
                winner,
                starting_dealer,
                Seat::new(RiichiVariant::Yonma, 3).expect("seat"),
                from,
            ]
        );
        assert_eq!(result.placements()[0].oka_tenths(), 200);
        assert_eq!(result.placements()[0].uma_tenths(), 300);
        assert_eq!(result.placements()[0].score_tenths(), 527);
        assert_eq!(game.hands().len(), 1);
    }

    #[test]
    fn last_dealer_win_ends_only_when_agari_yame_conditions_hold() {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let from = Seat::new(RiichiVariant::Yonma, 1).expect("from");
        let mut leader = RiichiMatch::start(rules.clone(), dealer).expect("match");
        set_hand(
            &mut leader,
            Wind::South,
            4,
            dealer,
            &[40_000, 20_000, 20_000, 20_000],
            0,
        );
        let hand = HandSettlement
            .settle(
                &rules,
                leader.progress(),
                leader.points().iter().copied(),
                HandOutcome::Ron {
                    from,
                    winners: vec![ScoredWinner::new(
                        dealer,
                        evaluation(Payment::Ron { points: 1_500 }),
                    )]
                    .into_boxed_slice(),
                },
            )
            .expect("hand");
        assert_eq!(
            leader
                .apply_hand(hand)
                .expect("apply")
                .expect("agari yame")
                .end_reason(),
            MatchEndReason::AgariYame
        );

        let mut trailing = RiichiMatch::start(rules.clone(), dealer).expect("match");
        set_hand(
            &mut trailing,
            Wind::South,
            4,
            dealer,
            &[20_000, 40_000, 20_000, 20_000],
            0,
        );
        let hand = HandSettlement
            .settle(
                &rules,
                trailing.progress(),
                trailing.points().iter().copied(),
                HandOutcome::Ron {
                    from,
                    winners: vec![ScoredWinner::new(
                        dealer,
                        evaluation(Payment::Ron { points: 1_500 }),
                    )]
                    .into_boxed_slice(),
                },
            )
            .expect("hand");
        assert!(trailing.apply_hand(hand).expect("apply").is_none());
        assert_eq!(trailing.progress().round_wind(), Wind::South);
        assert_eq!(trailing.progress().round_number().value(), 4);
        assert_eq!(trailing.progress().honba().value(), 1);
    }

    #[test]
    fn tobi_ends_before_scheduled_round() {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let winner = Seat::new(RiichiVariant::Yonma, 1).expect("winner");
        let from = Seat::new(RiichiVariant::Yonma, 2).expect("from");
        let mut game = RiichiMatch::start(rules.clone(), dealer).expect("match");
        set_hand(
            &mut game,
            Wind::East,
            1,
            dealer,
            &[33_300, 33_300, 100, 33_300],
            0,
        );
        let hand = HandSettlement
            .settle(
                &rules,
                game.progress(),
                game.points().iter().copied(),
                HandOutcome::Ron {
                    from,
                    winners: vec![ScoredWinner::new(
                        winner,
                        evaluation(Payment::Ron { points: 1_000 }),
                    )]
                    .into_boxed_slice(),
                },
            )
            .expect("hand");

        assert_eq!(
            game.apply_hand(hand)
                .expect("apply")
                .expect("tobi")
                .end_reason(),
            MatchEndReason::Tobi
        );
    }

    #[test]
    fn terminal_draw_awards_unclaimed_sticks_to_deterministic_leader() {
        let mut rules = RiichiRules::default();
        rules.match_rules.length = MatchLength::EastOnly;
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let mut game = RiichiMatch::start(rules.clone(), dealer).expect("match");
        set_hand(
            &mut game,
            Wind::East,
            4,
            dealer,
            &[24_000, 25_000, 25_000, 25_000],
            1,
        );
        let hand = HandSettlement
            .settle(
                &rules,
                game.progress(),
                game.points().iter().copied(),
                HandOutcome::ExhaustiveDraw {
                    tenpai: Box::new([]),
                    nagashi_winners: Box::new([]),
                },
            )
            .expect("hand");

        let result = game
            .apply_hand(hand)
            .expect("apply")
            .expect("scheduled end");

        assert_eq!(result.unclaimed_riichi_sticks_awarded(), 1);
        assert_eq!(result.final_points(), [24_000, 26_000, 25_000, 25_000]);
        assert_eq!(result.placements()[0].seat().index(), 1);
    }

    #[test]
    fn mismatched_hand_is_rejected_without_mutating_match() {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let mut game = RiichiMatch::start(rules.clone(), dealer).expect("match");
        let before = game.clone();
        let hand = HandSettlement
            .settle(
                &rules,
                game.progress(),
                [30_000; 4],
                HandOutcome::AbortiveDraw {
                    reason: crate::EndReason::NineTerminals,
                },
            )
            .expect("hand");

        assert!(game.apply_hand(hand).is_err());
        assert_eq!(game, before);
    }

    #[test]
    fn jpml_a_floating_uma_covers_every_valid_floating_count() {
        let rules = RiichiPreset::JpmlA.rules();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let cases = [
            ([30_000, 30_000, 30_000, 30_000], [0, 0, 0, 0]),
            ([40_000, 28_000, 27_000, 25_000], [12, -1, -3, -8]),
            ([35_000, 31_000, 29_000, 25_000], [8, 4, -4, -8]),
            ([31_000, 31_000, 31_000, 27_000], [8, 3, 1, -12]),
        ];

        for (points, expected) in cases {
            let order = ranking_order(&points, dealer, RiichiVariant::Yonma);
            assert_eq!(
                placement_uma(&rules, &points, &order).expect("valid floating state"),
                expected
            );
        }
    }
}
