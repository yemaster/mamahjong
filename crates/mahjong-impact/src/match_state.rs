//! 单节（也就是整场）的点数与杠点账。
//!
//! 冲击麻将整场只打一节：四家各 100 点、0 杠点起步，东起庄，庄家和牌连庄、
//! 闲家和牌庄位轮转，荒牌本局不算、同一庄重开。胜者向其余三家各收和牌点数，
//! 谁不够付就把剩下的点数全给胜者；点数下限是 0，**杠点可以为负、不设下限**。
//! 只要有人点数归零，整场立刻结束进入结算。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::config::{
    ALL_IN_WINNER_POINTS, INITIAL_KAN_POINTS, INITIAL_POINTS, ImpactRules, SEAT_COUNT,
};
use crate::hand::{EndReason, ImpactHand};
use crate::progress::{ProgressError, Seat, TableProgress};
use crate::scoring::WinEvaluation;
use crate::wall::WallSeed;

const SEATS: usize = SEAT_COUNT as usize;

/// 一局打完之后的账。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandSettlement {
    reason: EndReason,
    winner: Option<Seat>,
    evaluation: Option<WinEvaluation>,
    payer: Option<Seat>,
    chankan: bool,
    point_deltas: [i32; SEATS],
    kan_point_deltas: [i32; SEATS],
    points_after: [i32; SEATS],
    kan_points_after: [i32; SEATS],
    match_over: bool,
}

impl HandSettlement {
    #[must_use]
    pub const fn reason(&self) -> EndReason {
        self.reason
    }

    #[must_use]
    pub const fn winner(&self) -> Option<Seat> {
        self.winner
    }

    #[must_use]
    pub const fn evaluation(&self) -> Option<&WinEvaluation> {
        self.evaluation.as_ref()
    }

    #[must_use]
    pub const fn payer(&self) -> Option<Seat> {
        self.payer
    }

    #[must_use]
    pub const fn is_chankan(&self) -> bool {
        self.chankan
    }

    /// 本局点数增减。全交时是「胜者补到 400、其余三家扣到 0」的差值。
    #[must_use]
    pub const fn point_deltas(&self) -> &[i32; SEATS] {
        &self.point_deltas
    }

    #[must_use]
    pub const fn kan_point_deltas(&self) -> &[i32; SEATS] {
        &self.kan_point_deltas
    }

    #[must_use]
    pub const fn points_after(&self) -> &[i32; SEATS] {
        &self.points_after
    }

    #[must_use]
    pub const fn kan_points_after(&self) -> &[i32; SEATS] {
        &self.kan_points_after
    }

    /// 这一局打完之后整场是否结束（有人点数归零）。
    #[must_use]
    pub const fn match_over(&self) -> bool {
        self.match_over
    }

    /// 本局不算（荒牌），同一庄直接重开。
    #[must_use]
    pub const fn is_void(&self) -> bool {
        matches!(self.reason, EndReason::ExhaustiveDraw)
    }
}

/// 整场结束时每一家的成绩。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerResult {
    pub seat: Seat,
    pub points: i32,
    /// 相对 100 点起始分的增减。
    pub point_delta: i32,
    pub kan_points: i32,
}

#[derive(Clone, Debug)]
pub struct ImpactMatch {
    rules: ImpactRules,
    progress: TableProgress,
    points: [i32; SEATS],
    kan_points: [i32; SEATS],
    hand: Option<ImpactHand>,
    finished: bool,
}

impl ImpactMatch {
    /// 开桌：四家各 100 点、0 杠点，东家坐庄，还没有开局。
    #[must_use]
    pub fn new(rules: ImpactRules, dealer: Seat) -> Self {
        Self {
            rules,
            progress: TableProgress::opening(dealer),
            points: [INITIAL_POINTS; SEATS],
            kan_points: [INITIAL_KAN_POINTS; SEATS],
            hand: None,
            finished: false,
        }
    }

    #[must_use]
    pub const fn rules(&self) -> &ImpactRules {
        &self.rules
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn points(&self) -> &[i32; SEATS] {
        &self.points
    }

    #[must_use]
    pub const fn kan_points(&self) -> &[i32; SEATS] {
        &self.kan_points
    }

    #[must_use]
    pub const fn hand(&self) -> Option<&ImpactHand> {
        self.hand.as_ref()
    }

    pub fn hand_mut(&mut self) -> Option<&mut ImpactHand> {
        self.hand.as_mut()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// 整场结算：每家的点数、相对起始分的增减，以及杠点。
    #[must_use]
    pub fn results(&self) -> [PlayerResult; SEATS] {
        Seat::all().map(|seat| PlayerResult {
            seat,
            points: self.points[slot(seat)],
            point_delta: self.points[slot(seat)] - INITIAL_POINTS,
            kan_points: self.kan_points[slot(seat)],
        })
    }

    /// 开一局。
    ///
    /// # Errors
    ///
    /// 整场已经结束，或者上一局还没结算。
    pub fn start_hand(&mut self, seed: &WallSeed) -> Result<(), MatchError> {
        if self.finished {
            return Err(MatchError::MatchFinished);
        }
        if self.hand.as_ref().is_some_and(|h| !h.phase().is_ended()) {
            return Err(MatchError::HandInProgress);
        }
        self.hand = Some(ImpactHand::new(
            self.rules,
            self.progress.dealer(),
            self.progress.dealer_streak().value(),
            seed,
        ));
        Ok(())
    }

    /// 结算刚打完的一局：过杠点、过点数、推进庄位，并判断整场是否结束。
    ///
    /// # Errors
    ///
    /// 现在没有牌局，或者这一局还没打完。
    pub fn settle_hand(&mut self) -> Result<HandSettlement, MatchError> {
        let hand = self.hand.as_ref().ok_or(MatchError::NoHandInProgress)?;
        let outcome = hand.outcome().ok_or(MatchError::HandNotFinished)?.clone();
        // 保留手牌数据——结算期 view() 还要靠它拿各家手牌、
        // 财神指示牌和剩张。牌局的 phase 已经是 Ended，
        // start_hand() 里只拦非 Ended 的手，不会挡着开下一局。

        let kan_point_deltas = *outcome.kan_point_deltas();
        for seat in Seat::all() {
            self.kan_points[slot(seat)] += kan_point_deltas[slot(seat)];
        }

        let point_deltas = match (outcome.winner(), outcome.evaluation()) {
            (Some(winner), Some(evaluation)) => {
                self.pay_winner_for(winner, evaluation, outcome.payer(), outcome.is_chankan())
            }
            // 荒牌：本局不算，点数不动、庄位不动。
            _ => [0; SEATS],
        };
        for seat in Seat::all() {
            self.points[slot(seat)] += point_deltas[slot(seat)];
        }

        if let Some(winner) = outcome.winner() {
            if winner == self.progress.dealer() {
                self.progress.continue_dealership()?;
            } else {
                self.progress.rotate_dealership();
            }
        }

        // 有人点数归零 → 整场结束。
        self.finished = self.points.iter().any(|points| *points <= 0);

        Ok(HandSettlement {
            reason: outcome.reason(),
            winner: outcome.winner(),
            evaluation: outcome.evaluation().cloned(),
            payer: outcome.payer(),
            chankan: outcome.is_chankan(),
            point_deltas,
            kan_point_deltas,
            points_after: self.points,
            kan_points_after: self.kan_points,
            match_over: self.finished,
        })
    }

    /// 算出本局的点数增减，但先不落账。
    #[cfg(test)]
    fn pay_winner(&self, winner: Seat, evaluation: &WinEvaluation) -> [i32; SEATS] {
        self.pay_winner_for(winner, evaluation, None, false)
    }

    fn pay_winner_for(
        &self,
        winner: Seat,
        evaluation: &WinEvaluation,
        payer: Option<Seat>,
        chankan: bool,
    ) -> [i32; SEATS] {
        let mut deltas = [0; SEATS];

        if evaluation.is_all_in() {
            // 全交：胜者变 400，其余三家变 0。
            for seat in Seat::all() {
                let target = if seat == winner {
                    ALL_IN_WINNER_POINTS
                } else {
                    0
                };
                deltas[slot(seat)] = target - self.points[slot(seat)];
            }
            return deltas;
        }

        let value = i32::try_from(evaluation.points()).unwrap_or(i32::MAX);
        let mut collected = 0;
        for seat in Seat::all() {
            if seat == winner {
                continue;
            }
            let owed = match payer {
                None => value,
                Some(discarder) if chankan && seat == discarder => value.saturating_mul(3),
                Some(discarder) if seat == discarder => value,
                Some(_) if chankan => 0,
                Some(_) => value.saturating_add(1) / 2,
            };
            // 付不起就把剩下的点数全给胜者，点数不为负。
            let paid = owed.min(self.points[slot(seat)].max(0));
            deltas[slot(seat)] = -paid;
            collected += paid;
        }
        deltas[slot(winner)] = collected;
        deltas
    }
}

const fn slot(seat: Seat) -> usize {
    seat.index() as usize
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchError {
    /// 上一局还没结算。
    HandInProgress,
    /// 现在没有牌局。
    NoHandInProgress,
    /// 这一局还没打完。
    HandNotFinished,
    /// 整场已经结束。
    MatchFinished,
    Progress(ProgressError),
}

impl Display for MatchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandInProgress => write!(formatter, "the previous hand is not settled yet"),
            Self::NoHandInProgress => write!(formatter, "no hand is in progress"),
            Self::HandNotFinished => write!(formatter, "the hand is still running"),
            Self::MatchFinished => write!(formatter, "the match is already over"),
            Self::Progress(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MatchError {}

impl From<ProgressError> for MatchError {
    fn from(error: ProgressError) -> Self {
        Self::Progress(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{ImpactMatch, MatchError, SEATS};
    use crate::config::{ALL_IN_WINNER_POINTS, INITIAL_POINTS, ImpactRules};
    use crate::hand::EndReason;
    use crate::progress::Seat;
    use crate::scoring::{AllInKind, WinEvaluation};
    use crate::wall::WallSeed;

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn table() -> ImpactMatch {
        ImpactMatch::new(ImpactRules::standard(), seat(0))
    }

    #[test]
    fn every_seat_starts_at_a_hundred_points_and_no_kan_points() {
        let table = table();

        assert_eq!(table.points(), &[INITIAL_POINTS; SEATS]);
        assert_eq!(table.kan_points(), &[0; SEATS]);
        assert_eq!(table.progress().dealer(), seat(0));
        assert_eq!(table.progress().dealer_streak().value(), 0);
        assert!(!table.is_finished());
    }

    #[test]
    fn a_hand_must_be_settled_before_the_next_one_starts() {
        let mut table = table();
        let seed = WallSeed::from_bytes([3; 32]);

        table.start_hand(&seed).expect("the first hand starts");

        assert_eq!(table.start_hand(&seed), Err(MatchError::HandInProgress));
    }

    #[test]
    fn an_unfinished_hand_cannot_be_settled() {
        let mut table = table();
        assert_eq!(table.settle_hand(), Err(MatchError::NoHandInProgress));

        table
            .start_hand(&WallSeed::from_bytes([5; 32]))
            .expect("the first hand starts");

        assert_eq!(table.settle_hand(), Err(MatchError::HandNotFinished));
    }

    #[test]
    fn a_dealer_win_collects_from_everyone_and_keeps_the_seat() {
        let table = table();
        let deltas = table.pay_winner(seat(0), &win(20));

        assert_eq!(deltas, [60, -20, -20, -20]);
    }

    #[test]
    fn bright_ron_charges_the_discarder_full_and_the_others_half_rounded_up() {
        let table = table();
        let deltas = table.pay_winner_for(seat(0), &win(13), Some(seat(1)), false);

        assert_eq!(deltas, [27, -13, -7, -7]);
    }

    #[test]
    fn chankan_charges_only_the_kan_declarer_three_times_the_hand_value() {
        let table = table();
        let deltas = table.pay_winner_for(seat(2), &win(13), Some(seat(0)), true);

        assert_eq!(deltas, [-39, 0, 39, 0]);
    }

    #[test]
    fn a_payer_who_cannot_afford_the_hand_gives_away_the_rest() {
        let mut table = table();
        table.points = [100, 8, 100, 100];

        let deltas = table.pay_winner(seat(0), &win(20));

        assert_eq!(deltas, [48, -8, -20, -20], "付不起就把剩下的全给胜者");
        assert_eq!(table.points[1] + deltas[1], 0, "点数不为负");
    }

    #[test]
    fn an_all_in_sets_the_winner_to_four_hundred_and_the_rest_to_zero() {
        let mut table = table();
        table.points = [90, 130, 80, 100];

        let deltas = table.pay_winner(seat(2), &all_in());

        assert_eq!(deltas, [-90, -130, ALL_IN_WINNER_POINTS - 80, -100]);
    }

    #[test]
    fn zero_points_ends_the_whole_match() {
        let mut table = table();
        table.points = [100, 3, 100, 100];
        table.finish_hand_with(
            EndReason::Tsumo,
            Some(seat(0)),
            Some(win(20)),
            [1, -1, 0, 0],
        );

        let settlement = table.settle_hand().expect("the hand settles");

        assert!(settlement.match_over());
        assert!(table.is_finished());
        assert_eq!(settlement.points_after(), &[143, 0, 80, 80]);
        assert_eq!(settlement.kan_point_deltas(), &[1, -1, 0, 0]);
        assert_eq!(table.kan_points(), &[1, -1, 0, 0]);
        assert_eq!(
            table.start_hand(&WallSeed::from_bytes([9; 32])),
            Err(MatchError::MatchFinished)
        );
    }

    #[test]
    fn a_non_dealer_win_rotates_the_dealership() {
        let mut table = table();
        table.finish_hand_with(EndReason::Tsumo, Some(seat(1)), Some(win(12)), [0; SEATS]);
        table.settle_hand().expect("the hand settles");

        assert_eq!(table.progress().dealer(), seat(1));
        assert_eq!(table.progress().dealer_streak().value(), 0);
    }

    #[test]
    fn a_dealer_win_counts_the_streak() {
        let mut table = table();
        table.finish_hand_with(EndReason::Tsumo, Some(seat(0)), Some(win(12)), [0; SEATS]);
        table.settle_hand().expect("the hand settles");

        assert_eq!(table.progress().dealer(), seat(0));
        assert_eq!(table.progress().dealer_streak().value(), 1);
    }

    #[test]
    fn an_exhausted_wall_voids_the_hand_and_keeps_the_dealer() {
        let mut table = table();
        table.finish_hand_with(EndReason::ExhaustiveDraw, None, None, [2, -2, 0, 0]);

        let settlement = table.settle_hand().expect("the hand settles");

        assert!(settlement.is_void());
        assert_eq!(settlement.point_deltas(), &[0; SEATS]);
        assert_eq!(table.points(), &[INITIAL_POINTS; SEATS]);
        assert_eq!(table.kan_points(), &[2, -2, 0, 0], "杠点照旧结算");
        assert_eq!(table.progress().dealer(), seat(0));
        assert_eq!(table.progress().dealer_streak().value(), 0);
    }

    #[test]
    fn kan_points_may_go_negative_without_a_floor() {
        let mut table = table();
        for _ in 0..5 {
            table.finish_hand_with(EndReason::ExhaustiveDraw, None, None, [6, -2, -2, -2]);
            table.settle_hand().expect("the hand settles");
        }

        assert_eq!(table.kan_points(), &[30, -10, -10, -10]);
    }

    #[test]
    fn the_final_results_report_the_delta_against_the_starting_points() {
        let mut table = table();
        table.points = [143, 0, 80, 77];
        table.kan_points = [4, -1, -3, 0];

        let results = table.results();

        assert_eq!(results[0].point_delta, 43);
        assert_eq!(results[1].points, 0);
        assert_eq!(results[3].point_delta, -23);
        assert_eq!(results[2].kan_points, -3);
    }

    // ---- 测试辅助 ----

    fn win(points: u32) -> WinEvaluation {
        WinEvaluation::for_test(points)
    }

    fn all_in() -> WinEvaluation {
        WinEvaluation::from_trigger(AllInKind::ThreeKans)
    }

    impl ImpactMatch {
        /// 直接摆一个打完的牌局，绕开整局流程。
        fn finish_hand_with(
            &mut self,
            reason: EndReason,
            winner: Option<Seat>,
            evaluation: Option<WinEvaluation>,
            kan_point_deltas: [i32; SEATS],
        ) {
            let mut hand = crate::hand::ImpactHand::new(
                self.rules,
                self.progress.dealer(),
                self.progress.dealer_streak().value(),
                &WallSeed::from_bytes([11; 32]),
            );
            hand.force_outcome(reason, winner, evaluation, kan_point_deltas);
            self.hand = Some(hand);
        }
    }
}
