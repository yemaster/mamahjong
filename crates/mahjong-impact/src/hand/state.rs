//! 单局状态机。
//!
//! 冲击麻将没有吃、没有荣和、没有立直、没有振听。一局里能做的事只有：
//! 摸牌 → 自摸 / 暗杠 / 加杠 / 指示牌暗杠 / 打牌，以及对别家打出的牌碰 / 明杠 / 过。
//!
//! 杠点是一本独立于点数的账，**可以为负、不设下限**，本局的增减由
//! [`ImpactHand::kan_point_deltas`] 报给上层。

use crate::config::{ImpactRules, SEAT_COUNT};
use crate::hand::model::{
    Discard, DrawSource, EndReason, HandError, HandPhase, Meld, MeldId, MeldKind, ReactionKind,
};
use crate::progress::Seat;
use crate::scoring::{AllInKind, MeldSummary, WinContext, WinEvaluation, evaluate};
use crate::tile::{Tile, TileId, TileKind};
use crate::wall::{Wall, WallSeed};

const SEATS: usize = SEAT_COUNT as usize;
const STARTING_TILES: usize = 13;

/// 明杠 / 指示牌碰：被鸣的那一家付这么多杠点。
const OPEN_KAN_KAN_POINTS: i32 = 3;
/// 暗杠 / 指示牌暗杠：其余三家各付这么多。
const CONCEALED_KAN_KAN_POINTS: i32 = 2;
/// 加杠「仅单人支付」时，被碰那一家付这么多。
const ADDED_KAN_SINGLE_KAN_POINTS: i32 = 3;
/// 加杠三家分摊时，每家付这么多。
const ADDED_KAN_SHARED_KAN_POINTS: i32 = 1;
/// 第一巡连打：庄家向其余三家各付这么多。
const FIRST_ROUND_REPEAT_KAN_POINTS: i32 = 1;
/// 打出四张相同牌：向另外三人各收这么多（非连续）。
const FOUR_IDENTICAL_KAN_POINTS: i32 = 1;
/// 连续打出四张相同牌：向另外三人各收这么多（连续，加倍）。
const FOUR_CONSECUTIVE_KAN_POINTS: i32 = 2;

/// 新杠点种类——由 discard 触发，上层用来填 `ObserverKanPoints.kind`。
pub const KAN_FOUR_IDENTICAL_DISCARDS: &str = "four_identical_discards";
pub const KAN_FOUR_CONSECUTIVE_DISCARDS: &str = "four_consecutive_discards";
pub const KAN_THREE_INDICATOR_DISCARDS: &str = "three_indicator_discards";
pub const KAN_THREE_CONSECUTIVE_INDICATOR: &str = "three_consecutive_indicator";
/// 连打多少张字牌触发全交。
const HONOR_STREAK_FOR_ALL_IN: u32 = 11;
/// 几个真杠触发全交。
const KANS_FOR_ALL_IN: u8 = 3;
/// 几张财神触发全交。
const JOKERS_FOR_ALL_IN: u8 = 4;

/// 轮到自己时可以做的事。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnAction {
    Discard {
        tile: TileId,
    },
    Tsumo,
    ConcealedKan {
        tile: TileKind,
    },
    AddedKan {
        meld: MeldId,
    },
    /// 手持三张指示牌的暗杠：只结算杠点，牌型仍是刻子。
    IndicatorConcealedKan,
}

/// 轮到自己时的合法选项，供上层直接投影给前端。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnActions {
    pub can_tsumo: bool,
    pub concealed_kans: Vec<TileKind>,
    pub added_kans: Vec<MeldId>,
    pub indicator_concealed_kan: bool,
    /// 打哪张能听牌：(打出的那张牌的 TileId, 打出后所有能和的牌种)。
    /// 纯信息字段，不影响 `is_empty` 的语义。
    pub tenpai_discard_hints: Vec<(TileId, Vec<TileKind>)>,
}

impl TurnActions {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.can_tsumo
            && self.concealed_kans.is_empty()
            && self.added_kans.is_empty()
            && !self.indicator_concealed_kan
    }
}

/// 对别家打出的牌可以做的事。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReactionOptions {
    pub can_pon: bool,
    pub can_open_kan: bool,
    /// 这次碰会按「指示牌碰」结算杠点。
    pub pon_is_indicator: bool,
}

impl ReactionOptions {
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.can_pon && !self.can_open_kan
    }
}

/// 一家的手牌。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerHand {
    concealed: Vec<Tile>,
    /// 刚摸到的那一张，仅供展示；牌已经在 `concealed` 里。
    drawn: Option<TileId>,
    melds: Vec<Meld>,
    discards: Vec<Discard>,
    /// 真杠数（指示牌碰 / 指示牌暗杠不计）。
    kan_count: u8,
    /// 自己连续打出的字牌或财神数。
    honor_streak: u32,
}

impl PlayerHand {
    fn new() -> Self {
        Self {
            concealed: Vec::with_capacity(14),
            drawn: None,
            melds: Vec::new(),
            discards: Vec::new(),
            kan_count: 0,
            honor_streak: 0,
        }
    }

    #[must_use]
    pub fn concealed(&self) -> &[Tile] {
        &self.concealed
    }

    #[must_use]
    pub const fn drawn(&self) -> Option<TileId> {
        self.drawn
    }

    #[must_use]
    pub fn melds(&self) -> &[Meld] {
        &self.melds
    }

    #[must_use]
    pub fn discards(&self) -> &[Discard] {
        &self.discards
    }

    #[must_use]
    pub const fn kan_count(&self) -> u8 {
        self.kan_count
    }

    #[must_use]
    pub const fn honor_streak(&self) -> u32 {
        self.honor_streak
    }

    fn insert(&mut self, tile: Tile) {
        let position = self
            .concealed
            .partition_point(|held| sort_key(*held) <= sort_key(tile));
        self.concealed.insert(position, tile);
    }

    fn count_of(&self, kind: TileKind) -> u8 {
        u8::try_from(
            self.concealed
                .iter()
                .filter(|held| held.kind() == kind)
                .count(),
        )
        .expect("a hand holds at most 14 tiles")
    }

    fn remove_id(&mut self, tile: TileId) -> Result<Tile, HandError> {
        let position = self
            .concealed
            .iter()
            .position(|held| held.id() == tile)
            .ok_or(HandError::TileNotHeld { tile })?;
        Ok(self.concealed.remove(position))
    }

    fn remove_kind(&mut self, kind: TileKind, count: u8) -> Result<Vec<Tile>, HandError> {
        if self.count_of(kind) < count {
            return Err(HandError::MeldNotAvailable);
        }
        let mut taken = Vec::with_capacity(usize::from(count));
        while taken.len() < usize::from(count) {
            let position = self
                .concealed
                .iter()
                .position(|held| held.kind() == kind)
                .expect("count was checked above");
            taken.push(self.concealed.remove(position));
        }
        Ok(taken)
    }

    /// 手上（含副露）这个牌种一共有几张。
    fn total_of(&self, kind: TileKind) -> u8 {
        let melded: u8 = self
            .melds
            .iter()
            .filter(|meld| meld.tile() == kind)
            .map(|meld| meld.kind().tile_count())
            .sum();
        self.count_of(kind) + melded
    }

    fn meld_summaries(&self) -> Vec<MeldSummary> {
        self.melds
            .iter()
            .map(|meld| MeldSummary::new(meld.kind(), meld.tile()))
            .collect()
    }

    fn concealed_kinds(&self) -> Vec<TileKind> {
        self.concealed.iter().copied().map(Tile::kind).collect()
    }
}

fn sort_key(tile: Tile) -> (usize, u16) {
    (tile.kind().index(), tile.id().value())
}

/// 座位在各条按座位索引的数组里的下标。
const fn slot(seat: Seat) -> usize {
    seat.index() as usize
}

/// 单局结果。点数结算交给上层（要看各家现有点数才能封顶）。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandOutcome {
    reason: EndReason,
    winner: Option<Seat>,
    evaluation: Option<WinEvaluation>,
    kan_point_deltas: [i32; SEATS],
}

impl HandOutcome {
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
    pub const fn kan_point_deltas(&self) -> &[i32; SEATS] {
        &self.kan_point_deltas
    }
}

/// 第一巡连打的追踪状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepeatDiscardStreak {
    tile: TileKind,
    /// 下一个必须打出同一张牌的座位。
    next: Seat,
    /// 还差几家。
    remaining: u8,
}

#[derive(Clone, Debug)]
pub struct ImpactHand {
    rules: ImpactRules,
    wall: Wall,
    joker: TileKind,
    indicator: Tile,
    dealer: Seat,
    dealer_streak: u32,
    players: [PlayerHand; SEATS],
    phase: HandPhase,
    kan_point_deltas: [i32; SEATS],
    last_discard: Option<(Seat, Tile)>,
    reaction_eligible: [bool; SEATS],
    reaction_answers: [Option<ReactionKind>; SEATS],
    next_meld_id: u16,
    outcome: Option<HandOutcome>,
    repeat_discard: Option<RepeatDiscardStreak>,
    /// 已经进行过的正常摸牌回合数（岭上牌不计），用来判天和地和。
    turns_taken: u32,
    /// 出现过任何鸣牌 / 杠，天和地和作废。
    interrupted: bool,
    /// discard 触发的杠点变动是哪种，上层取完即清。
    discard_kan_kind: Option<&'static str>,
    /// 刚摸的那张是不是牌山最后一张（海底）。
    last_draw_was_final: bool,
    /// 刚摸的那张是不是岭上牌（杠上开花）。
    last_draw_was_replacement: bool,
}

impl ImpactHand {
    /// 发牌开局：每家 13 张，庄家再摸第 14 张。
    #[must_use]
    pub fn new(rules: ImpactRules, dealer: Seat, dealer_streak: u32, seed: &WallSeed) -> Self {
        let mut wall = Wall::new(dealer, seed);
        let joker = wall.joker();
        let indicator = wall.indicator();

        let mut players = [
            PlayerHand::new(),
            PlayerHand::new(),
            PlayerHand::new(),
            PlayerHand::new(),
        ];
        for round in 0..STARTING_TILES {
            for offset in 0..SEAT_COUNT {
                let seat = dealer.offset_by(offset);
                let tile = wall
                    .draw()
                    .expect("a fresh wall always holds the opening tiles");
                players[slot(seat)].insert(tile);
                let _ = round;
            }
        }

        let mut hand = Self {
            rules,
            wall,
            joker,
            indicator,
            dealer,
            dealer_streak,
            players,
            phase: HandPhase::Ended {
                reason: EndReason::ExhaustiveDraw,
            },
            kan_point_deltas: [0; SEATS],
            last_discard: None,
            reaction_eligible: [false; SEATS],
            reaction_answers: [None; SEATS],
            next_meld_id: 0,
            outcome: None,
            repeat_discard: None,
            turns_taken: 0,
            interrupted: false,
            discard_kan_kind: None,
            last_draw_was_final: false,
            last_draw_was_replacement: false,
        };
        hand.draw_and_open_turn(dealer, DrawSource::Wall);
        hand
    }

    #[must_use]
    pub const fn rules(&self) -> &ImpactRules {
        &self.rules
    }

    #[must_use]
    pub const fn joker(&self) -> TileKind {
        self.joker
    }

    /// 财神指示牌（左上角唯一展示的那一张）。
    #[must_use]
    pub const fn indicator(&self) -> Tile {
        self.indicator
    }

    #[must_use]
    pub const fn dealer(&self) -> Seat {
        self.dealer
    }

    #[must_use]
    pub const fn dealer_streak(&self) -> u32 {
        self.dealer_streak
    }

    #[must_use]
    pub const fn phase(&self) -> HandPhase {
        self.phase
    }

    #[must_use]
    pub const fn player(&self, seat: Seat) -> &PlayerHand {
        &self.players[slot(seat)]
    }

    #[must_use]
    pub const fn remaining_draws(&self) -> usize {
        self.wall.remaining_draws()
    }

    #[must_use]
    pub const fn kan_point_deltas(&self) -> &[i32; SEATS] {
        &self.kan_point_deltas
    }

    /// discard 触发的杠点变动是哪种，取完即清（幂等）。
    pub fn take_discard_kan_kind(&mut self) -> Option<&'static str> {
        self.discard_kan_kind.take()
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<&HandOutcome> {
        self.outcome.as_ref()
    }

    #[must_use]
    pub const fn last_discard(&self) -> Option<(Seat, Tile)> {
        self.last_discard
    }

    /// 还没表态的座位。
    #[must_use]
    pub fn pending_reactions(&self) -> Vec<Seat> {
        Seat::all()
            .into_iter()
            .filter(|seat| {
                self.reaction_eligible[slot(*seat)] && self.reaction_answers[slot(*seat)].is_none()
            })
            .collect()
    }

    #[must_use]
    pub fn turn_actions(&self, seat: Seat) -> TurnActions {
        if self.phase != (HandPhase::AwaitingTurnAction { seat }) {
            return TurnActions::default();
        }
        let player = &self.players[slot(seat)];
        let has_replacement = self.wall.remaining_draws() > 0;

        let concealed_kans = if has_replacement {
            let mut kinds: Vec<TileKind> = Vec::new();
            for tile in player.concealed() {
                let kind = tile.kind();
                // 财神不能被碰、被杠：它只在和牌时当别的牌用，摆不成副露。
                if kind != self.joker && player.count_of(kind) == 4 && !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
            kinds
        } else {
            Vec::new()
        };

        let added_kans = if has_replacement {
            player
                .melds()
                .iter()
                .filter(|meld| {
                    meld.kind() == MeldKind::Pon
                        && meld.tile() != self.joker
                        && player.count_of(meld.tile()) > 0
                })
                .map(Meld::id)
                .collect()
        } else {
            Vec::new()
        };

        let indicator_kind = self.indicator.kind();
        let indicator_concealed_kan = self.rules.kan.indicator_pon_counts_as_kan
            && player.count_of(indicator_kind) >= 3
            // 四张齐了走真暗杠，不走指示牌暗杠。
            && player.count_of(indicator_kind) < 4;

        TurnActions {
            can_tsumo: self.tsumo_evaluation(seat).is_some(),
            concealed_kans,
            added_kans,
            indicator_concealed_kan,
            tenpai_discard_hints: self.tenpai_discard_hints(seat),
        }
    }

    #[must_use]
    pub fn reaction_options(&self, seat: Seat) -> ReactionOptions {
        let HandPhase::AwaitingResponses { discarder } = self.phase else {
            return ReactionOptions::default();
        };
        if seat == discarder || self.reaction_answers[slot(seat)].is_some() {
            return ReactionOptions::default();
        }
        let Some((_, tile)) = self.last_discard else {
            return ReactionOptions::default();
        };
        self.reaction_options_for(seat, tile.kind())
    }

    fn reaction_options_for(&self, seat: Seat, kind: TileKind) -> ReactionOptions {
        // 打出来的是财神，谁也鸣不了：财神只做百搭，不进副露。
        if kind == self.joker {
            return ReactionOptions::default();
        }
        // 海底：最后四张牌各人摸一张，这个阶段不许副露。
        if self.wall.remaining_draws() <= 4 {
            return ReactionOptions::default();
        }
        let held = self.players[slot(seat)].count_of(kind);
        ReactionOptions {
            can_pon: held >= 2,
            can_open_kan: held >= 3 && self.wall.remaining_draws() > 0,
            pon_is_indicator: self.rules.kan.indicator_pon_counts_as_kan
                && kind == self.indicator.kind(),
        }
    }

    /// # Errors
    ///
    /// 座位不对、当前阶段不接受这个动作、牌不在手上、凑不齐副露、或者本局已经结束。
    pub fn apply_turn_action(&mut self, seat: Seat, action: TurnAction) -> Result<(), HandError> {
        match self.phase {
            HandPhase::Ended { .. } => return Err(HandError::HandAlreadyEnded),
            HandPhase::AwaitingTurnAction { seat: current } if current == seat => {}
            HandPhase::AwaitingDiscard { seat: current } if current == seat => {
                if !matches!(action, TurnAction::Discard { .. }) {
                    return Err(HandError::UnexpectedAction);
                }
            }
            _ => return Err(HandError::OutOfTurn { seat }),
        }

        match action {
            TurnAction::Discard { tile } => self.discard(seat, tile),
            TurnAction::Tsumo => self.tsumo(seat),
            TurnAction::ConcealedKan { tile } => self.concealed_kan(seat, tile),
            TurnAction::AddedKan { meld } => self.added_kan(seat, meld),
            TurnAction::IndicatorConcealedKan => self.indicator_concealed_kan(seat),
        }
    }

    /// # Errors
    ///
    /// 现在不在等响应、这个座位没有表态资格、已经表过态，或者选了不可用的选项。
    pub fn apply_reaction(&mut self, seat: Seat, kind: ReactionKind) -> Result<(), HandError> {
        let HandPhase::AwaitingResponses { discarder } = self.phase else {
            if self.phase.is_ended() {
                return Err(HandError::HandAlreadyEnded);
            }
            return Err(HandError::UnexpectedAction);
        };
        if !self.reaction_eligible[slot(seat)] {
            return Err(HandError::OutOfTurn { seat });
        }
        if self.reaction_answers[slot(seat)].is_some() {
            return Err(HandError::UnexpectedAction);
        }

        let (_, tile) = self.last_discard.ok_or(HandError::UnexpectedAction)?;
        let options = self.reaction_options_for(seat, tile.kind());
        match kind {
            ReactionKind::Pon if !options.can_pon => return Err(HandError::MeldNotAvailable),
            ReactionKind::OpenKan if !options.can_open_kan => {
                return Err(HandError::MeldNotAvailable);
            }
            _ => {}
        }

        self.reaction_answers[slot(seat)] = Some(kind);
        if self.pending_reactions().is_empty() {
            self.resolve_reactions(discarder);
        }
        Ok(())
    }

    fn discard(&mut self, seat: Seat, tile: TileId) -> Result<(), HandError> {
        let tile = self.players[slot(seat)].remove_id(tile)?;
        let player = &mut self.players[slot(seat)];
        player.drawn = None;
        player.discards.push(Discard::new(tile));

        if tile.kind().is_honor() || tile.kind() == self.joker {
            player.honor_streak += 1;
        } else {
            player.honor_streak = 0;
        }
        let streak = player.honor_streak;

        self.last_draw_was_replacement = false;
        self.last_draw_was_final = false;

        if self.rules.all_in.eleven_honor_streak && streak >= HONOR_STREAK_FOR_ALL_IN {
            self.finish_with_trigger(seat, AllInKind::ElevenHonorStreak);
            return Ok(());
        }

        self.track_repeat_discard(seat, tile.kind());
        self.check_four_identical_as_kan(seat, tile.kind());
        self.last_discard = Some((seat, tile));
        self.open_reactions(seat, tile.kind());
        Ok(())
    }

    fn tsumo(&mut self, seat: Seat) -> Result<(), HandError> {
        let evaluation = self
            .tsumo_evaluation(seat)
            .ok_or(HandError::NotAWinningHand)?;
        self.finish(EndReason::Tsumo, Some(seat), Some(evaluation));
        Ok(())
    }

    fn tsumo_evaluation(&self, seat: Seat) -> Option<WinEvaluation> {
        let player = &self.players[slot(seat)];
        let drawn = player.drawn?;
        let winning_tile = player
            .concealed
            .iter()
            .find(|tile| tile.id() == drawn)?
            .kind();
        let concealed = player.concealed_kinds();
        let melds = player.meld_summaries();

        evaluate(&WinContext {
            rules: &self.rules,
            joker: self.joker,
            concealed: &concealed,
            melds: &melds,
            winning_tile,
            dealer_streak: self.dealer_streak,
            rinshan: self.last_draw_was_replacement,
            last_tile: self.last_draw_was_final,
            blessing: !self.interrupted && self.turns_taken <= u32::from(SEAT_COUNT),
            honor_streak: player.honor_streak,
        })
    }

    /// 「打哪张能听」：有摸牌时，对每一种可以打出的牌种，返回打出后所有能和的牌种。
    /// 副露后等出牌（无 drawn）时返回空。
    #[must_use]
    fn tenpai_discard_hints(&self, seat: Seat) -> Vec<(TileId, Vec<TileKind>)> {
        let player = &self.players[slot(seat)];
        // 只在有摸牌的14张状态下计算；副露后的 AwaitingDiscard 不在此处处理。
        if player.drawn.is_none() {
            return Vec::new();
        }
        let melds = player.meld_summaries();
        let all_kinds = player.concealed_kinds();
        let kind_count = u8::try_from(TileKind::COUNT).expect("TileKind::COUNT fits u8");

        let mut seen: Vec<TileKind> = Vec::new();
        let mut hints = Vec::new();

        for tile in &player.concealed {
            let discard_kind = tile.kind();
            if seen.contains(&discard_kind) {
                continue;
            }
            seen.push(discard_kind);

            // 模拟打出第一张此牌种
            let mut remaining = all_kinds.clone();
            let pos = remaining
                .iter()
                .position(|&k| k == discard_kind)
                .expect("tile is in concealed hand");
            remaining.remove(pos);

            // 遍历全部 34 种牌码，找出打出后能和的候选牌
            let waiting: Vec<TileKind> = (0..kind_count)
                .map(|index| TileKind::from_index(index).expect("valid index"))
                .filter(|&candidate| {
                    let mut test = remaining.clone();
                    test.push(candidate);
                    evaluate(&WinContext {
                        rules: &self.rules,
                        joker: self.joker,
                        concealed: &test,
                        melds: &melds,
                        winning_tile: candidate,
                        dealer_streak: self.dealer_streak,
                        rinshan: false,
                        last_tile: false,
                        blessing: false,
                        honor_streak: player.honor_streak,
                    })
                    .is_some()
                })
                .collect();

            if !waiting.is_empty() {
                hints.push((tile.id(), waiting));
            }
        }

        hints
    }

    fn concealed_kan(&mut self, seat: Seat, kind: TileKind) -> Result<(), HandError> {
        if kind == self.joker {
            // 四张财神走的是「四龙全交」那条路，不是暗杠。
            return Err(HandError::MeldNotAvailable);
        }
        if self.wall.remaining_draws() == 0 {
            return Err(HandError::WallExhausted);
        }
        let tiles = self.players[slot(seat)].remove_kind(kind, 4)?;
        self.push_meld(seat, MeldKind::ConcealedKan, kind, tiles, None, None);

        for other in Seat::all() {
            if other != seat {
                self.transfer_kan_points(seat, other, CONCEALED_KAN_KAN_POINTS);
            }
        }
        self.after_kan(seat)
    }

    fn indicator_concealed_kan(&mut self, seat: Seat) -> Result<(), HandError> {
        if !self.rules.kan.indicator_pon_counts_as_kan {
            return Err(HandError::UnexpectedAction);
        }
        let kind = self.indicator.kind();
        let tiles = self.players[slot(seat)].remove_kind(kind, 3)?;
        self.push_meld(seat, MeldKind::IndicatorConcealed, kind, tiles, None, None);

        for other in Seat::all() {
            if other != seat {
                self.transfer_kan_points(seat, other, CONCEALED_KAN_KAN_POINTS);
            }
        }
        // 牌型仍是刻子：不摸岭上牌、不算杠上开花、不计入三杠。
        self.interrupted = true;
        self.repeat_discard = None;
        self.phase = HandPhase::AwaitingDiscard { seat };
        Ok(())
    }

    fn added_kan(&mut self, seat: Seat, meld_id: MeldId) -> Result<(), HandError> {
        if self.wall.remaining_draws() == 0 {
            return Err(HandError::WallExhausted);
        }
        let player = &self.players[slot(seat)];
        let meld = player
            .melds
            .iter()
            .find(|meld| meld.id() == meld_id)
            .ok_or(HandError::MeldNotFound { meld: meld_id })?;
        if meld.kind() != MeldKind::Pon || meld.tile() == self.joker {
            return Err(HandError::MeldNotAvailable);
        }
        let kind = meld.tile();
        let source = meld.called_from();

        let tile = self.players[slot(seat)]
            .remove_kind(kind, 1)?
            .pop()
            .expect("remove_kind returned the requested tile");
        let player = &mut self.players[slot(seat)];
        let meld = player
            .melds
            .iter_mut()
            .find(|meld| meld.id() == meld_id)
            .expect("the meld was found above");
        meld.upgrade_to_added_kan(tile);
        player.kan_count += 1;
        player.drawn = None;

        if self.rules.kan.added_kan_single_payer {
            if let Some(payer) = source {
                self.transfer_kan_points(seat, payer, ADDED_KAN_SINGLE_KAN_POINTS);
            }
        } else {
            for other in Seat::all() {
                if other != seat {
                    self.transfer_kan_points(seat, other, ADDED_KAN_SHARED_KAN_POINTS);
                }
            }
        }

        self.interrupted = true;
        self.repeat_discard = None;
        if self.finish_kan_triggers(seat) {
            return Ok(());
        }
        // 加杠也先等四家把杠点动画播完，再摸岭上牌。
        self.phase = HandPhase::AwaitingKanAnimation { seat };
        Ok(())
    }

    fn open_reactions(&mut self, discarder: Seat, kind: TileKind) {
        self.reaction_answers = [None; SEATS];
        self.reaction_eligible = [false; SEATS];

        let mut any = false;
        for seat in Seat::all() {
            if seat == discarder {
                continue;
            }
            let eligible = !self.reaction_options_for(seat, kind).is_empty();
            self.reaction_eligible[slot(seat)] = eligible;
            any |= eligible;
        }

        if any {
            self.phase = HandPhase::AwaitingResponses { discarder };
        } else {
            self.advance_turn(discarder);
        }
    }

    fn resolve_reactions(&mut self, discarder: Seat) {
        let mut chosen: Option<(Seat, ReactionKind)> = None;
        for offset in 1..SEAT_COUNT {
            let seat = discarder.offset_by(offset);
            match self.reaction_answers[slot(seat)] {
                Some(ReactionKind::OpenKan) => {
                    chosen = Some((seat, ReactionKind::OpenKan));
                    break;
                }
                Some(ReactionKind::Pon) if chosen.is_none() => {
                    chosen = Some((seat, ReactionKind::Pon));
                }
                _ => {}
            }
        }

        let Some((seat, kind)) = chosen else {
            self.advance_turn(discarder);
            return;
        };
        self.execute_call(discarder, seat, kind);
    }

    fn execute_call(&mut self, discarder: Seat, caller: Seat, kind: ReactionKind) {
        let (_, called_tile) = self.last_discard.expect("a call always answers a discard");
        let tile_kind = called_tile.kind();

        if let Some(discard) = self.players[slot(discarder)].discards.last_mut() {
            discard.mark_called();
        }

        let from_hand = match kind {
            ReactionKind::OpenKan => 3,
            _ => 2,
        };
        let mut tiles = self.players[slot(caller)]
            .remove_kind(tile_kind, from_hand)
            .expect("eligibility was checked when the reaction was accepted");
        tiles.push(called_tile);

        let indicator_pon = self.rules.kan.indicator_pon_counts_as_kan
            && tile_kind == self.indicator.kind()
            && kind == ReactionKind::Pon;
        let meld_kind = match kind {
            ReactionKind::OpenKan => MeldKind::OpenKan,
            _ if indicator_pon => MeldKind::IndicatorPon,
            _ => MeldKind::Pon,
        };

        self.push_meld(
            caller,
            meld_kind,
            tile_kind,
            tiles,
            Some(discarder),
            Some(called_tile.id()),
        );

        self.last_discard = None;
        self.reaction_eligible = [false; SEATS];
        self.reaction_answers = [None; SEATS];
        self.interrupted = true;
        self.repeat_discard = None;

        // 手牌 ≦4 张时碰牌收杠点：明碰收 3，明杠收 6。
        // 判定的是「鸣牌之前」的手牌数，不是鸣完之后剩几张。
        let few_tiles = self.rules.kan.pon_with_few_tiles_as_kan
            && self.players[slot(caller)].concealed.len() + from_hand as usize <= 4;
        if few_tiles && kind == ReactionKind::Pon && !indicator_pon {
            // 普通碰牌但手牌很少 → 收 3 杠点
            self.transfer_kan_points(caller, discarder, OPEN_KAN_KAN_POINTS);
        }

        // 明杠与指示牌碰都按「被鸣的那一家付 3」结算。
        if meld_kind == MeldKind::OpenKan || meld_kind == MeldKind::IndicatorPon {
            let amount = if few_tiles && meld_kind == MeldKind::OpenKan {
                OPEN_KAN_KAN_POINTS * 2 // 手牌少时明杠收双倍
            } else {
                OPEN_KAN_KAN_POINTS
            };
            self.transfer_kan_points(caller, discarder, amount);
        }

        if self.finish_kan_triggers(caller) {
            return;
        }

        if meld_kind == MeldKind::OpenKan {
            // 明杠也先等四家播杠点动画，再摸岭上牌。
            self.phase = HandPhase::AwaitingKanAnimation { seat: caller };
        } else {
            self.phase = HandPhase::AwaitingDiscard { seat: caller };
        }
    }

    fn push_meld(
        &mut self,
        seat: Seat,
        kind: MeldKind,
        tile: TileKind,
        tiles: Vec<Tile>,
        called_from: Option<Seat>,
        called_tile: Option<TileId>,
    ) {
        let id = MeldId::new(self.next_meld_id);
        self.next_meld_id += 1;
        let player = &mut self.players[slot(seat)];
        player.drawn = None;
        if kind.is_kan() {
            player.kan_count += 1;
        }
        player
            .melds
            .push(Meld::new(id, kind, tile, tiles, called_from, called_tile));
    }

    /// 杠之后统一处理：三杠 / 四龙的立即全交。返回 `true` 表示本局已经结束。
    fn finish_kan_triggers(&mut self, seat: Seat) -> bool {
        if self.rules.all_in.three_kans && self.players[slot(seat)].kan_count >= KANS_FOR_ALL_IN {
            self.finish_with_trigger(seat, AllInKind::ThreeKans);
            return true;
        }
        if self.rules.all_in.four_jokers && self.joker_total(seat) >= JOKERS_FOR_ALL_IN {
            self.finish_with_trigger(seat, AllInKind::FourJokers);
            return true;
        }
        false
    }

    fn after_kan(&mut self, seat: Seat) -> Result<(), HandError> {
        self.interrupted = true;
        self.repeat_discard = None;
        if self.finish_kan_triggers(seat) {
            return Ok(());
        }
        // 先等四家把杠点动画播完，播完再由上层调 advance_from_kan_animation 摸岭上牌。
        self.phase = HandPhase::AwaitingKanAnimation { seat };
        Ok(())
    }

    /// 杠点动画四家都播完后，由 `ImpactRuntime` 调用，为 `seat` 摸岭上牌并开始新回合。
    ///
    /// # Errors
    ///
    /// 当前阶段不是 `AwaitingKanAnimation`，或座位不匹配。
    pub fn advance_from_kan_animation(&mut self, seat: Seat) -> Result<(), HandError> {
        match self.phase {
            HandPhase::AwaitingKanAnimation { seat: waiting } if waiting == seat => {}
            _ => return Err(HandError::UnexpectedAction),
        }
        self.draw_and_open_turn(seat, DrawSource::Replacement);
        Ok(())
    }

    fn joker_total(&self, seat: Seat) -> u8 {
        self.players[slot(seat)].total_of(self.joker)
    }

    fn advance_turn(&mut self, from: Seat) {
        self.last_discard = None;
        self.reaction_eligible = [false; SEATS];
        self.reaction_answers = [None; SEATS];
        self.draw_and_open_turn(from.next(), DrawSource::Wall);
    }

    fn draw_and_open_turn(&mut self, seat: Seat, source: DrawSource) {
        let drawn = match source {
            DrawSource::Wall => self.wall.draw(),
            DrawSource::Replacement => self.wall.draw_from_back().ok(),
        };
        let Some(tile) = drawn else {
            self.finish(EndReason::ExhaustiveDraw, None, None);
            return;
        };

        if source == DrawSource::Wall {
            self.turns_taken += 1;
        }
        self.last_draw_was_replacement = source == DrawSource::Replacement;
        self.last_draw_was_final = self.wall.remaining_draws() == 0;

        let player = &mut self.players[slot(seat)];
        player.insert(tile);
        player.drawn = Some(tile.id());

        if self.rules.all_in.four_jokers && self.joker_total(seat) >= JOKERS_FOR_ALL_IN {
            self.finish_with_trigger(seat, AllInKind::FourJokers);
            return;
        }

        self.phase = HandPhase::AwaitingTurnAction { seat };
    }

    /// 打出四张相同牌算杠：牌河中四张相同牌向另外三人各收 1 杠点；
    /// 连续四张（末尾无其他牌）则各收 2 杠点。指示牌碰杠开启后，三张指示牌
    /// 同样触发（连打三张收双倍）。每到达一次阈值（如 4, 8, 12…）触一次。
    fn check_four_identical_as_kan(&mut self, seat: Seat, kind: TileKind) {
        if !self.rules.kan.four_identical_discards_as_kan {
            return;
        }

        let is_indicator =
            self.rules.kan.indicator_pon_counts_as_kan && kind == self.indicator.kind();
        let needed: usize = if is_indicator { 3 } else { 4 };

        let same_kind = self.players[slot(seat)]
            .discards
            .iter()
            .filter(|d| d.tile().kind() == kind)
            .count();

        if same_kind == 0 || same_kind % needed != 0 {
            return;
        }

        let consecutive = self.players[slot(seat)]
            .discards
            .iter()
            .rev()
            .take(needed)
            .all(|d| d.tile().kind() == kind);

        let amount = if consecutive {
            FOUR_CONSECUTIVE_KAN_POINTS
        } else {
            FOUR_IDENTICAL_KAN_POINTS
        };

        for other in Seat::all() {
            if other != seat {
                self.transfer_kan_points(seat, other, amount);
            }
        }

        self.discard_kan_kind = Some(match (is_indicator, consecutive) {
            (false, false) => KAN_FOUR_IDENTICAL_DISCARDS,
            (false, true) => KAN_FOUR_CONSECUTIVE_DISCARDS,
            (true, false) => KAN_THREE_INDICATOR_DISCARDS,
            (true, true) => KAN_THREE_CONSECUTIVE_INDICATOR,
        });
    }

    /// 第一巡连打：庄家第一张打出后，三家无人鸣牌且依次打出同一种牌。
    /// 如果同时开启了指示牌碰算杠且庄家第一张打的是指示牌，则改为追踪指示牌：
    /// 第一巡 3/4 人打出指示牌（不必同种），庄家向其余三家各付 1 杠点。
    fn track_repeat_discard(&mut self, seat: Seat, kind: TileKind) {
        if !self.rules.kan.first_round_repeat_discard {
            return;
        }

        match self.repeat_discard {
            None => {
                let first_discard = Seat::all().into_iter().all(|other| {
                    self.players[slot(other)].discards.len() == usize::from(other == seat)
                });
                if seat == self.dealer && first_discard && !self.interrupted {
                    let indicator_mode =
                        self.rules.kan.indicator_pon_counts_as_kan && kind == self.indicator.kind();
                    let (track_kind, needed) = if indicator_mode {
                        // 追踪指示牌：只需要再有 2 人打出任意指示牌（共 3 人）
                        (self.indicator.kind(), SEAT_COUNT - 2)
                    } else {
                        (kind, SEAT_COUNT - 1)
                    };
                    self.repeat_discard = Some(RepeatDiscardStreak {
                        tile: track_kind,
                        next: seat.next(),
                        remaining: needed,
                    });
                }
            }
            Some(streak) => {
                if streak.next != seat || streak.tile != kind {
                    self.repeat_discard = None;
                    return;
                }
                let remaining = streak.remaining - 1;
                if remaining == 0 {
                    self.repeat_discard = None;
                    for other in Seat::all() {
                        if other != self.dealer {
                            self.transfer_kan_points(
                                other,
                                self.dealer,
                                FIRST_ROUND_REPEAT_KAN_POINTS,
                            );
                        }
                    }
                } else {
                    self.repeat_discard = Some(RepeatDiscardStreak {
                        next: seat.next(),
                        remaining,
                        ..streak
                    });
                }
            }
        }
    }

    fn transfer_kan_points(&mut self, receiver: Seat, payer: Seat, amount: i32) {
        self.kan_point_deltas[slot(receiver)] += amount;
        self.kan_point_deltas[slot(payer)] -= amount;
    }

    fn finish_with_trigger(&mut self, seat: Seat, kind: AllInKind) {
        self.finish(
            EndReason::Tsumo,
            Some(seat),
            Some(WinEvaluation::from_trigger(kind)),
        );
    }

    fn finish(
        &mut self,
        reason: EndReason,
        winner: Option<Seat>,
        evaluation: Option<WinEvaluation>,
    ) {
        self.last_discard = None;
        self.reaction_eligible = [false; SEATS];
        self.reaction_answers = [None; SEATS];
        self.repeat_discard = None;
        self.phase = HandPhase::Ended { reason };
        self.outcome = Some(HandOutcome {
            reason,
            winner,
            evaluation,
            kan_point_deltas: self.kan_point_deltas,
        });
    }

    /// 只给测试用：直接摆一个打完的牌局，绕开整局流程。
    #[cfg(test)]
    pub(crate) fn force_outcome(
        &mut self,
        reason: EndReason,
        winner: Option<Seat>,
        evaluation: Option<WinEvaluation>,
        kan_point_deltas: [i32; SEATS],
    ) {
        self.kan_point_deltas = kan_point_deltas;
        self.finish(reason, winner, evaluation);
    }
}

#[cfg(test)]
mod tests {
    use super::{ImpactHand, ReactionKind, TurnAction, slot};
    use crate::config::ImpactRules;
    use crate::hand::model::{EndReason, HandPhase, MeldKind};
    use crate::progress::Seat;
    use crate::scoring::AllInKind;
    use crate::tile::{Tile, TileKind};
    use crate::wall::WallSeed;

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn seed(byte: u8) -> WallSeed {
        WallSeed::from_bytes([byte; 32])
    }

    fn opening() -> ImpactHand {
        ImpactHand::new(ImpactRules::standard(), seat(0), 0, &seed(7))
    }

    /// 打掉当前手上第一张不会被任何人碰到的牌，把回合推进下去。
    fn discard_first(hand: &mut ImpactHand, actor: Seat) {
        let tile = hand.player(actor).concealed()[0];
        hand.apply_turn_action(actor, TurnAction::Discard { tile: tile.id() })
            .expect("discard is legal");
    }

    /// 打掉手上第一张指定牌种的牌。
    fn discard_kind(hand: &mut ImpactHand, actor: Seat, kind: TileKind) {
        let tile = *hand
            .player(actor)
            .concealed()
            .iter()
            .find(|tile| tile.kind() == kind)
            .expect("the tile was planted");
        hand.apply_turn_action(actor, TurnAction::Discard { tile: tile.id() })
            .expect("discard is legal");
    }

    fn pass_all(hand: &mut ImpactHand) {
        for pending in hand.pending_reactions() {
            hand.apply_reaction(pending, ReactionKind::Pass)
                .expect("pass is always legal");
        }
    }

    #[test]
    fn opening_deals_thirteen_tiles_and_one_draw_for_the_dealer() {
        let hand = opening();

        assert_eq!(hand.player(seat(0)).concealed().len(), 14);
        for index in 1..4 {
            assert_eq!(hand.player(seat(index)).concealed().len(), 13);
        }
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingTurnAction { seat: seat(0) }
        );
        assert!(hand.player(seat(0)).drawn().is_some());
        // 136 张减去 52 张起手、1 张庄家摸的、翻财神那一墩的 2 张。
        assert_eq!(hand.remaining_draws(), 136 - 52 - 1 - 2);
    }

    #[test]
    fn the_joker_follows_the_indicator() {
        let hand = opening();

        assert_eq!(hand.joker(), crate::tile::joker_of(hand.indicator().kind()));
    }

    #[test]
    fn play_passes_to_the_next_seat_when_nobody_calls() {
        let mut hand = opening();
        discard_first(&mut hand, seat(0));
        pass_all(&mut hand);

        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingTurnAction { seat: seat(1) }
        );
    }

    #[test]
    fn out_of_turn_actions_are_rejected() {
        let mut hand = opening();
        let tile = hand.player(seat(1)).concealed()[0];

        assert!(
            hand.apply_turn_action(seat(1), TurnAction::Discard { tile: tile.id() })
                .is_err()
        );
    }

    #[test]
    fn a_concealed_kan_charges_the_other_three_two_points_each() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        set_hand(&mut hand, seat(0), &spread(&kinds, &[4, 4, 3, 3]));

        hand.apply_turn_action(seat(0), TurnAction::ConcealedKan { tile: kinds[0] })
            .expect("a concealed kan is legal on four identical tiles");

        assert_eq!(hand.kan_point_deltas(), &[6, -2, -2, -2]);
        assert_eq!(
            hand.player(seat(0)).melds()[0].kind(),
            MeldKind::ConcealedKan
        );
        assert_eq!(hand.player(seat(0)).kan_count(), 1);
        // 杠完先进入动画等待阶段，还没摸岭上牌。
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingKanAnimation { seat: seat(0) }
        );
    }

    #[test]
    fn an_open_kan_charges_the_discarder_three_points() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        set_hand(&mut hand, seat(0), &spread(&kinds, &[1, 5, 4, 4]));
        set_hand(&mut hand, seat(1), &spread(&kinds, &[3, 4, 3, 3]));

        discard_kind(&mut hand, seat(0), kinds[0]);
        hand.apply_reaction(seat(1), ReactionKind::OpenKan)
            .expect("an open kan is legal on three identical tiles");
        pass_all(&mut hand);

        assert_eq!(hand.kan_point_deltas(), &[-3, 3, 0, 0]);
        assert_eq!(hand.player(seat(1)).melds()[0].kind(), MeldKind::OpenKan);
    }

    #[test]
    fn a_pon_on_the_indicator_settles_like_an_open_kan_but_stays_a_triplet() {
        let mut hand = opening();
        let indicator = hand.indicator().kind();
        let kinds = plain_kinds(&hand, 3);
        let mut dealer = vec![indicator];
        dealer.extend(spread(&kinds, &[5, 4, 4]));
        let mut caller = vec![indicator, indicator];
        caller.extend(spread(&kinds, &[4, 4, 3]));
        set_hand(&mut hand, seat(0), &dealer);
        set_hand(&mut hand, seat(1), &caller);

        discard_kind(&mut hand, seat(0), indicator);
        hand.apply_reaction(seat(1), ReactionKind::Pon)
            .expect("a pon is legal on two identical tiles");
        pass_all(&mut hand);

        let meld = &hand.player(seat(1)).melds()[0];
        assert_eq!(meld.kind(), MeldKind::IndicatorPon);
        assert_eq!(meld.tiles().len(), 3, "指示牌碰仍然是刻子");
        assert_eq!(hand.kan_point_deltas(), &[-3, 3, 0, 0]);
        assert_eq!(hand.player(seat(1)).kan_count(), 0, "指示牌碰不计入三杠");
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingDiscard { seat: seat(1) },
            "指示牌碰不摸岭上牌"
        );
    }

    #[test]
    fn a_discarded_joker_cannot_be_called() {
        let mut hand = opening();
        let joker = hand.joker();
        let kinds = plain_kinds(&hand, 3);
        let mut dealer = vec![joker];
        dealer.extend(spread(&kinds, &[5, 4, 4]));
        // 三张财神也照样鸣不了——碰、明杠都不行。
        let mut caller = vec![joker, joker, joker];
        caller.extend(spread(&kinds, &[4, 3, 3]));
        set_hand(&mut hand, seat(0), &dealer);
        set_hand(&mut hand, seat(1), &caller);

        discard_kind(&mut hand, seat(0), joker);

        let options = hand.reaction_options(seat(1));
        assert!(!options.can_pon, "财神不能被碰");
        assert!(!options.can_open_kan, "财神不能被明杠");
        assert!(hand.apply_reaction(seat(1), ReactionKind::Pon).is_err());
        assert!(hand.apply_reaction(seat(1), ReactionKind::OpenKan).is_err());
    }

    #[test]
    fn a_joker_cannot_stand_in_for_a_tile_when_calling() {
        let mut hand = opening();
        let joker = hand.joker();
        let kinds = plain_kinds(&hand, 3);
        set_hand(&mut hand, seat(0), &spread(&kinds, &[5, 5, 4]));
        // 一张真牌 + 一张财神：财神不能顶那张缺的牌去碰。
        let mut caller = vec![kinds[0], joker];
        caller.extend(spread(&kinds[1..], &[6, 5]));
        set_hand(&mut hand, seat(1), &caller);

        discard_kind(&mut hand, seat(0), kinds[0]);

        let options = hand.reaction_options(seat(1));
        assert!(!options.can_pon, "财神顶不上那张缺的牌");
        assert!(hand.apply_reaction(seat(1), ReactionKind::Pon).is_err());
    }

    #[test]
    fn four_jokers_in_hand_are_not_a_concealed_kan() {
        let mut hand = opening();
        let joker = hand.joker();
        let kinds = plain_kinds(&hand, 3);
        let mut dealer = vec![joker, joker, joker, joker];
        dealer.extend(spread(&kinds, &[4, 4, 2]));
        set_hand(&mut hand, seat(0), &dealer);

        let actions = hand.turn_actions(seat(0));
        assert!(!actions.concealed_kans.contains(&joker), "财神不能被暗杠");
        assert_eq!(actions.concealed_kans.len(), 2, "别的四张牌照样能暗杠");
        assert!(
            hand.apply_turn_action(seat(0), TurnAction::ConcealedKan { tile: joker })
                .is_err()
        );
    }

    #[test]
    fn the_first_round_repeat_discard_bills_the_dealer() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        let repeated = kinds[0];
        plant_single(&mut hand, repeated, &kinds[1..]);

        for index in 0..4 {
            discard_kind(&mut hand, seat(index), repeated);
            pass_all(&mut hand);
        }

        assert_eq!(hand.kan_point_deltas(), &[-3, 1, 1, 1]);
    }

    #[test]
    fn a_differing_discard_breaks_the_first_round_streak() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        let repeated = kinds[0];
        plant_single(&mut hand, repeated, &kinds[1..]);
        // 最后一家手上没有这张牌，连打被打断。
        set_hand(&mut hand, seat(3), &spread(&kinds[1..], &[5, 4, 4]));

        for index in 0..3 {
            discard_kind(&mut hand, seat(index), repeated);
            pass_all(&mut hand);
        }
        discard_kind(&mut hand, seat(3), kinds[1]);
        pass_all(&mut hand);

        assert_eq!(hand.kan_point_deltas(), &[0, 0, 0, 0]);
    }

    #[test]
    fn three_kans_end_the_hand_with_an_all_in() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        set_hand(&mut hand, seat(0), &spread(&kinds, &[4, 4, 4, 2]));

        for kind in kinds.iter().take(3) {
            hand.apply_turn_action(seat(0), TurnAction::ConcealedKan { tile: *kind })
                .expect("a concealed kan is legal on four identical tiles");
            // 第三杠直接全交结束，局已经结束就不用再等动画了。
            if hand.outcome().is_none() {
                hand.advance_from_kan_animation(seat(0))
                    .expect("advancing after kan animation");
            }
        }

        let outcome = hand.outcome().expect("the hand ended");
        assert_eq!(outcome.reason(), EndReason::Tsumo);
        assert_eq!(outcome.winner(), Some(seat(0)));
        assert_eq!(
            outcome.evaluation().and_then(|win| win.all_in()),
            Some(AllInKind::ThreeKans)
        );
    }

    #[test]
    fn a_hand_that_ended_refuses_further_actions() {
        let mut hand = opening();
        let kinds = plain_kinds(&hand, 4);
        set_hand(&mut hand, seat(0), &spread(&kinds, &[4, 4, 4, 2]));
        for kind in kinds.iter().take(3) {
            hand.apply_turn_action(seat(0), TurnAction::ConcealedKan { tile: *kind })
                .expect("a concealed kan is legal on four identical tiles");
            if hand.outcome().is_none() {
                hand.advance_from_kan_animation(seat(0))
                    .expect("advancing after kan animation");
            }
        }

        let tile = hand.player(seat(0)).concealed()[0];
        assert!(
            hand.apply_turn_action(seat(0), TurnAction::Discard { tile: tile.id() })
                .is_err()
        );
    }

    // ---- 测试辅助：直接铺手牌，绕开洗牌的随机性 ----

    /// 造一张牌山里不存在的牌：id 落在真牌用不到的区段，保证不与真牌冲突。
    fn spare_tile(kind: TileKind, nth: usize) -> Tile {
        let id = crate::tile::TileId::new(
            u16::try_from(1000 + kind.index() * 16 + nth).expect("small id"),
        );
        Tile::new(id, kind)
    }

    /// 挑几种既不是财神、也不是指示牌的字牌，避开四龙与指示牌碰的特殊分支。
    fn plain_kinds(hand: &ImpactHand, count: usize) -> Vec<TileKind> {
        let kinds: Vec<TileKind> = ["1z", "2z", "3z", "4z", "5z", "6z", "7z"]
            .into_iter()
            .map(|code| code.parse().expect("valid tile code"))
            .filter(|kind| *kind != hand.joker() && *kind != hand.indicator().kind())
            .take(count)
            .collect();
        assert_eq!(kinds.len(), count, "字牌够挑");
        kinds
    }

    /// 按张数展开成一手牌，例如 `spread(&kinds, &[4, 4, 3, 3])` 得到 14 张。
    fn spread(kinds: &[TileKind], counts: &[usize]) -> Vec<TileKind> {
        kinds
            .iter()
            .zip(counts)
            .flat_map(|(kind, count)| std::iter::repeat_n(*kind, *count))
            .collect()
    }

    /// 直接替换一家的手牌。`drawn` 一并清掉：这些牌都不是摸上来的。
    fn set_hand(hand: &mut ImpactHand, seat: Seat, kinds: &[TileKind]) {
        let player = &mut hand.players[slot(seat)];
        player.concealed.clear();
        player.drawn = None;
        for (nth, kind) in kinds.iter().enumerate() {
            player.insert(spare_tile(*kind, nth));
        }
    }

    /// 四家各拿一张 `single`，其余用 `fillers` 填满，谁都碰不了 `single`。
    fn plant_single(hand: &mut ImpactHand, single: TileKind, fillers: &[TileKind]) {
        for index in 0..4 {
            let counts = if index == 0 { [5, 4, 4] } else { [4, 4, 4] };
            let mut tiles = vec![single];
            tiles.extend(spread(fillers, &counts));
            set_hand(hand, seat(index), &tiles);
        }
    }
}
