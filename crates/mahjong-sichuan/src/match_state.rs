//! 整场（4 局血战到底）的点数账。
//!
//! 每家从 0 分打起，杠（雨）与胡都即时结算，只会在「尚未胡牌」的家之间流转。
//! 一家胡后盖牌退出，其余继续，直到三家胡或牌山摸尽（流局）。流局时再查花猪、
//! 查大叫，把点差补进账里。首局庄 = 东，之后庄 = 上一局第一个胡者，4 局打完结算。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::config::{HAND_COUNT, INITIAL_POINTS, SEAT_COUNT, SichuanRules};
use crate::hand::{EndReason, SichuanHand, WinnerRecord};
use crate::progress::{ProgressError, Seat, TableProgress};
use crate::wall::WallSeed;

const SEATS: usize = SEAT_COUNT as usize;

/// 查花猪：手牌含三门者，赔其余未胡家各这么多分。
pub const FLOWER_PIG_POINTS: i32 = 8000;
/// 查大叫：未听牌者，赔每位听牌者这么多分。
pub const NOTEN_POINTS: i32 = 1000;

/// 流局时的查花猪 / 查大叫结算。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueSettlement {
    flower_pigs: Vec<Seat>,
    tenpai: Vec<Seat>,
    noten: Vec<Seat>,
    deltas: [i32; SEATS],
}

impl QueSettlement {
    /// 花猪（手牌含三门）。
    #[must_use]
    pub fn flower_pigs(&self) -> &[Seat] {
        &self.flower_pigs
    }

    /// 听牌的家。
    #[must_use]
    pub fn tenpai(&self) -> &[Seat] {
        &self.tenpai
    }

    /// 未听牌的家（含花猪）。
    #[must_use]
    pub fn noten(&self) -> &[Seat] {
        &self.noten
    }

    /// 查花猪 + 查大叫的点数变动。
    #[must_use]
    pub const fn deltas(&self) -> &[i32; SEATS] {
        &self.deltas
    }
}

/// 一局打完之后的账。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandSettlement {
    reason: EndReason,
    winners: Vec<WinnerRecord>,
    que: Option<QueSettlement>,
    point_deltas: [i32; SEATS],
    points_after: [i32; SEATS],
    match_over: bool,
}

impl HandSettlement {
    #[must_use]
    pub const fn reason(&self) -> EndReason {
        self.reason
    }

    /// 本局所有胡家，按胡牌顺序（血战到底一家胡后继续）。
    #[must_use]
    pub fn winners(&self) -> &[WinnerRecord] {
        &self.winners
    }

    /// 流局时的查花猪 / 查大叫；非流局为 `None`。
    #[must_use]
    pub const fn que(&self) -> Option<&QueSettlement> {
        self.que.as_ref()
    }

    /// 本局总点数变动（杠 + 胡 + 查）。
    #[must_use]
    pub const fn point_deltas(&self) -> &[i32; SEATS] {
        &self.point_deltas
    }

    #[must_use]
    pub const fn points_after(&self) -> &[i32; SEATS] {
        &self.points_after
    }

    /// 这一局打完之后整场是否结束（4 局打完）。
    #[must_use]
    pub const fn match_over(&self) -> bool {
        self.match_over
    }
}

/// 整场结束时每一家的成绩。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerResult {
    pub seat: Seat,
    pub points: i32,
    /// 相对 0 分起始分的增减。
    pub point_delta: i32,
}

#[derive(Clone, Debug)]
pub struct SichuanMatch {
    rules: SichuanRules,
    progress: TableProgress,
    points: [i32; SEATS],
    hand: Option<SichuanHand>,
    finished: bool,
}

impl SichuanMatch {
    /// 开桌：四家各 0 分，东家坐庄，还没有开局。
    #[must_use]
    pub fn new(rules: SichuanRules) -> Self {
        Self {
            rules,
            progress: TableProgress::opening(),
            points: [INITIAL_POINTS; SEATS],
            hand: None,
            finished: false,
        }
    }

    #[must_use]
    pub const fn rules(&self) -> &SichuanRules {
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
    pub const fn hand(&self) -> Option<&SichuanHand> {
        self.hand.as_ref()
    }

    pub fn hand_mut(&mut self) -> Option<&mut SichuanHand> {
        self.hand.as_mut()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// 整场结算：每家的点数、相对起始分的增减。
    #[must_use]
    pub fn results(&self) -> [PlayerResult; SEATS] {
        Seat::all().map(|seat| PlayerResult {
            seat,
            points: self.points[slot(seat)],
            point_delta: self.points[slot(seat)] - INITIAL_POINTS,
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
        if self
            .hand
            .as_ref()
            .is_some_and(|hand| !hand.phase().is_ended())
        {
            return Err(MatchError::HandInProgress);
        }
        self.hand = Some(SichuanHand::new(self.rules, self.progress.dealer(), seed));
        Ok(())
    }

    /// 结算刚打完的一局：过点数（含流局查花猪 / 查大叫）、推进庄位，并判断整场是否结束。
    ///
    /// # Errors
    ///
    /// 现在没有牌局，或者这一局还没打完。
    pub fn settle_hand(&mut self) -> Result<HandSettlement, MatchError> {
        let hand = self.hand.as_ref().ok_or(MatchError::NoHandInProgress)?;
        let reason = hand.reason().ok_or(MatchError::HandNotFinished)?;

        let mut point_deltas = *hand.point_deltas();
        let winners = hand.winners().to_vec();

        let que = (reason == EndReason::ExhaustiveDraw).then(|| self.compute_que_settlement(hand));
        if let Some(que) = &que {
            for (index, delta) in que.deltas.iter().copied().enumerate() {
                point_deltas[index] += delta;
            }
        }
        for seat in Seat::all() {
            self.points[slot(seat)] += point_deltas[slot(seat)];
        }

        // 庄 = 上一局第一个胡者；流局无胡家则庄不变。
        let first_winner = winners.first().map(WinnerRecord::seat);
        self.progress.advance(first_winner)?;

        self.finished = self.progress.hand_index() >= HAND_COUNT;

        Ok(HandSettlement {
            reason,
            winners,
            que,
            point_deltas,
            points_after: self.points,
            match_over: self.finished,
        })
    }

    fn compute_que_settlement(&self, hand: &SichuanHand) -> QueSettlement {
        let active: Vec<Seat> = Seat::all()
            .into_iter()
            .filter(|seat| !hand.won(*seat))
            .collect();

        let mut flower_pigs = Vec::new();
        let mut tenpai = Vec::new();
        let mut noten = Vec::new();
        for seat in &active {
            if hand.is_flower_pig(*seat) {
                flower_pigs.push(*seat);
                noten.push(*seat);
            } else if hand.is_tenpai(*seat) {
                tenpai.push(*seat);
            } else {
                noten.push(*seat);
            }
        }

        let mut deltas = [0_i32; SEATS];
        for pig in &flower_pigs {
            for other in &active {
                if other != pig {
                    deltas[slot(*pig)] -= FLOWER_PIG_POINTS;
                    deltas[slot(*other)] += FLOWER_PIG_POINTS;
                }
            }
        }
        for loser in &noten {
            for winner in &tenpai {
                deltas[slot(*loser)] -= NOTEN_POINTS;
                deltas[slot(*winner)] += NOTEN_POINTS;
            }
        }

        QueSettlement {
            flower_pigs,
            tenpai,
            noten,
            deltas,
        }
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
    use super::{FLOWER_PIG_POINTS, MatchError, SichuanMatch};
    use crate::config::{HAND_COUNT, INITIAL_POINTS, SichuanRules};
    use crate::hand::{EndReason, TurnAction};
    use crate::progress::Seat;
    use crate::tile::Suit;
    use crate::wall::WallSeed;

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn table() -> SichuanMatch {
        SichuanMatch::new(SichuanRules::standard())
    }

    fn kinds(spec: &str) -> Vec<String> {
        spec.split_whitespace().map(str::to_owned).collect()
    }

    fn seed(byte: u8) -> WallSeed {
        WallSeed::from_bytes([byte; 32])
    }

    /// 摆一个进入打牌阶段、还没结束的牌局（庄家已摸牌）。
    fn start_play(table: &mut SichuanMatch) {
        table.start_hand(&seed(11)).expect("hand starts");
        table
            .hand_mut()
            .expect("hand in progress")
            .force_start_play([Suit::Man, Suit::Pin, Suit::Sou, Suit::Man]);
    }

    #[test]
    fn every_seat_starts_at_zero_points_with_east_dealing() {
        let table = table();

        assert_eq!(table.points(), &[INITIAL_POINTS; 4]);
        assert_eq!(table.progress().dealer(), seat(0));
        assert_eq!(table.progress().hand_index(), 0);
        assert!(!table.is_finished());
    }

    #[test]
    fn a_hand_must_be_settled_before_the_next_one_starts() {
        let mut table = table();

        table.start_hand(&seed(3)).expect("the first hand starts");

        assert_eq!(table.start_hand(&seed(3)), Err(MatchError::HandInProgress));
    }

    #[test]
    fn an_unfinished_hand_cannot_be_settled() {
        let mut table = table();
        assert_eq!(table.settle_hand(), Err(MatchError::NoHandInProgress));

        table.start_hand(&seed(5)).expect("the first hand starts");

        assert_eq!(table.settle_hand(), Err(MatchError::HandNotFinished));
    }

    #[test]
    fn a_tsumo_win_settles_into_the_winner_and_points() {
        let mut table = table();
        start_play(&mut table);
        {
            let hand = table.hand_mut().expect("hand in progress");
            // 0 号定缺万，塞一手清一色筒子自摸。
            hand.set_concealed_tiles(seat(0), &kinds("1p 2p 3p 4p 5p 6p 7p 8p 9p 1p 1p 1p 2p 2p"))
                .expect("plant hand");
            hand.apply_turn_action(seat(0), TurnAction::Tsumo)
                .expect("tsumo");
            hand.force_end(EndReason::ThreeWinners);
        }

        let settlement = table.settle_hand().expect("the hand settles");

        assert_eq!(settlement.reason(), EndReason::ThreeWinners);
        assert_eq!(settlement.winners().len(), 1);
        assert_eq!(settlement.winners()[0].seat(), seat(0));
        // 清一色 3 + 自摸 1 = 4 番 → 8000 分，其余三家各付 8000。
        assert_eq!(settlement.point_deltas(), &[24000, -8000, -8000, -8000]);
        assert_eq!(table.points(), &[24000, -8000, -8000, -8000]);
    }

    #[test]
    fn a_void_hand_keeps_the_same_dealer() {
        let mut table = table();
        start_play(&mut table);
        table
            .hand_mut()
            .expect("hand in progress")
            .force_end(EndReason::ExhaustiveDraw);

        let settlement = table.settle_hand().expect("the hand settles");

        assert_eq!(settlement.reason(), EndReason::ExhaustiveDraw);
        assert!(settlement.winners().is_empty());
        assert_eq!(table.progress().dealer(), seat(0));
        assert_eq!(table.progress().hand_index(), 1);
        assert!(!table.is_finished());
    }

    #[test]
    fn flower_pigs_pay_each_other_non_winner_eight() {
        let mut table = table();
        start_play(&mut table);
        {
            let hand = table.hand_mut().expect("hand in progress");
            // 0 号（庄，14 张）三门花猪；其余三家各留定缺门、不花猪、未听。
            hand.set_concealed_tiles(seat(0), &kinds("1m 2m 3m 1p 2p 3p 1s 2s 3s 4m 5m 4p 5p 4s"))
                .expect("plant hand");
            hand.set_concealed_tiles(seat(1), &kinds("1p 2p 3p 4p 5p 6p 7p 8p 9p 1m 2m 3m 1m"))
                .expect("plant hand");
            hand.set_concealed_tiles(seat(2), &kinds("1s 2s 3s 4s 5s 6s 7s 8s 9s 1m 2m 3m 1m"))
                .expect("plant hand");
            hand.set_concealed_tiles(seat(3), &kinds("1m 2m 3m 4m 5m 6m 7m 8m 9m 1p 2p 3p 1p"))
                .expect("plant hand");
            hand.force_end(EndReason::ExhaustiveDraw);
        }

        let settlement = table.settle_hand().expect("the hand settles");

        let que = settlement.que().expect("a void hand runs the que check");
        assert_eq!(que.flower_pigs(), &[seat(0)]);
        assert!(que.tenpai().is_empty());
        assert_eq!(que.noten().len(), 4);
        assert_eq!(
            que.deltas(),
            &[
                -3 * FLOWER_PIG_POINTS,
                FLOWER_PIG_POINTS,
                FLOWER_PIG_POINTS,
                FLOWER_PIG_POINTS
            ]
        );
        assert_eq!(settlement.point_deltas(), &[-24000, 8000, 8000, 8000]);
    }

    #[test]
    fn the_match_finishes_after_four_hands() {
        let mut table = table();
        for _ in 0..HAND_COUNT {
            start_play(&mut table);
            table
                .hand_mut()
                .expect("hand in progress")
                .force_end(EndReason::ExhaustiveDraw);
            table.settle_hand().expect("the hand settles");
        }

        assert!(table.is_finished());
        assert_eq!(table.start_hand(&seed(9)), Err(MatchError::MatchFinished));
    }

    #[test]
    fn final_results_report_the_delta_against_zero() {
        let mut table = table();
        table.points = [24, -8, -8, -8];

        let results = table.results();

        assert_eq!(results[0].point_delta, 24);
        assert_eq!(results[1].points, -8);
    }
}
