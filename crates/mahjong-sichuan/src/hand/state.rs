//! 单局状态机（血战到底）。
//!
//! 一局牌的流程：发牌（各 13 张）→ 换三张 → 定缺 → 摸打。摸打阶段没有吃、没有振听；
//! 一家胡后不结束，胡者盖牌退出，其余继续，直到三家胡或牌山摸尽（流局）。
//! 杠（雨）与胡都即时结算点数，只会在「尚未胡牌」的家之间流转。
//!
//! 流局的查花猪 / 查大叫结算在 `match_state.rs`，这里只负责把摸打流程跑到
//! `EndReason::ExhaustiveDraw` 或 `EndReason::ThreeWinners`，并暴露各家最终暗牌、
//! 副露、定缺与是否听牌。

use crate::config::{SEAT_COUNT, SichuanRules};
use crate::hand::model::{
    Discard, DrawSource, EndReason, HandError, HandPhase, Meld, MeldId, MeldKind, ReactionKind,
};
use crate::progress::Seat;
use crate::scoring::{MeldSummary, WinContext, WinEvaluation, evaluate};
use crate::tile::{Suit, Tile, TileId, TileKind};
use crate::wall::{Dice, ExchangeDirection, Wall, WallSeed};

const SEATS: usize = SEAT_COUNT as usize;
const STARTING_TILES: usize = 13;
/// 天胡 / 地胡：庄家第一张摸牌（第 1 次摸牌）或第一家闲家的第一张摸牌（第 2 次摸牌）。
const BLESSING_MAX_TURNS: u32 = 2;
/// 三家胡即结束。
const WINNERS_TO_END: usize = 3;
/// 暗杠：其余各家各付这么多分。
const CONCEALED_KAN_POINTS: i32 = 2000;
/// 明杠：放杠者付这么多分。
const OPEN_KAN_POINTS: i32 = 2000;
/// 加杠：其余各家各付这么多分。
const ADDED_KAN_POINTS: i32 = 1000;

/// 轮到自己时可以做的事。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnAction {
    Discard { tile: TileId },
    Tsumo,
    ConcealedKan { tile: TileKind },
    AddedKan { meld: MeldId },
}

/// 轮到自己时的合法选项，供上层直接投影给前端。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnActions {
    pub can_tsumo: bool,
    pub concealed_kans: Vec<TileKind>,
    pub added_kans: Vec<MeldId>,
    /// 打哪张能听牌：(打出的那张牌的 TileId, 打出后所有能和的牌种)。
    /// 纯信息字段，不影响 `is_empty` 的语义。
    pub tenpai_discard_hints: Vec<(TileId, Vec<TileKind>)>,
}

impl TurnActions {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.can_tsumo && self.concealed_kans.is_empty() && self.added_kans.is_empty()
    }
}

/// 对别家打出的牌可以做的事。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReactionOptions {
    pub can_ron: bool,
    pub can_pon: bool,
    pub can_open_kan: bool,
}

impl ReactionOptions {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.can_ron && !self.can_pon && !self.can_open_kan
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
    /// 杠数（根）。碰不计。
    kan_count: u8,
}

impl PlayerHand {
    fn new() -> Self {
        Self {
            concealed: Vec::with_capacity(14),
            drawn: None,
            melds: Vec::new(),
            discards: Vec::new(),
            kan_count: 0,
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

    fn meld_summaries(&self) -> Vec<MeldSummary> {
        self.melds
            .iter()
            .map(|meld| {
                MeldSummary::new(
                    meld.kind(),
                    meld.tiles().iter().copied().map(Tile::kind).collect(),
                )
            })
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

/// 一次胡的结算记录。`deltas` 是这次胡本身造成的点数变动（不含杠）。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinnerRecord {
    seat: Seat,
    evaluation: WinEvaluation,
    is_tsumo: bool,
    payer: Option<Seat>,
    chankan: bool,
    deltas: [i32; SEATS],
}

impl WinnerRecord {
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn evaluation(&self) -> &WinEvaluation {
        &self.evaluation
    }

    #[must_use]
    pub const fn is_tsumo(&self) -> bool {
        self.is_tsumo
    }

    #[must_use]
    pub const fn payer(&self) -> Option<Seat> {
        self.payer
    }

    #[must_use]
    pub const fn is_chankan(&self) -> bool {
        self.chankan
    }

    #[must_use]
    pub const fn deltas(&self) -> &[i32; SEATS] {
        &self.deltas
    }
}

/// 一次杠（雨）的结算记录。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KanEvent {
    seat: Seat,
    kind: MeldKind,
    deltas: [i32; SEATS],
}

impl KanEvent {
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn kind(&self) -> MeldKind {
        self.kind
    }

    #[must_use]
    pub const fn deltas(&self) -> &[i32; SEATS] {
        &self.deltas
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingAddedKan {
    declarer: Seat,
    meld_id: MeldId,
    tile: Tile,
}

#[derive(Clone, Debug)]
pub struct SichuanHand {
    rules: SichuanRules,
    wall: Wall,
    dealer: Seat,
    players: [PlayerHand; SEATS],
    phase: HandPhase,
    /// 各家的定缺门（换三张后、打牌前填满）。
    que_suits: [Option<Suit>; SEATS],
    /// 换三张阶段各家的选择。
    exchange_answers: [Option<[TileId; 3]>; SEATS],
    last_discard: Option<(Seat, Tile)>,
    /// 上一次打出的牌是不是杠后的岭上牌（杠上炮判定）。
    last_discard_was_replacement: bool,
    pending_added_kan: Option<PendingAddedKan>,
    reaction_eligible: [bool; SEATS],
    reaction_answers: [Option<ReactionKind>; SEATS],
    next_meld_id: u16,
    /// 各家是否已经胡（胡者盖牌退出）。
    won: [bool; SEATS],
    /// 杠与胡的累计点数变动。
    point_deltas: [i32; SEATS],
    winners: Vec<WinnerRecord>,
    kan_events: Vec<KanEvent>,
    /// 已经进行过的正常摸牌次数（岭上牌不计），用来判天胡 / 地胡。
    turns_taken: u32,
    /// 出现过任何鸣牌 / 杠，天胡地胡作废。
    interrupted: bool,
    /// 刚摸的那张是不是牌山最后一张（海底）。
    last_draw_was_final: bool,
    /// 刚摸的那张是不是岭上牌（杠上开花）。
    last_draw_was_replacement: bool,
}

impl SichuanHand {
    /// 发牌开局：每家 13 张、庄家补摸第 14 张，进入换三张阶段。
    #[must_use]
    pub fn new(rules: SichuanRules, dealer: Seat, seed: &WallSeed) -> Self {
        let mut wall = Wall::new(dealer, seed);

        let mut players = [
            PlayerHand::new(),
            PlayerHand::new(),
            PlayerHand::new(),
            PlayerHand::new(),
        ];
        for _ in 0..STARTING_TILES {
            for offset in 0..SEAT_COUNT {
                let seat = dealer.offset_by(offset);
                let tile = wall
                    .draw()
                    .expect("a fresh wall always holds the opening tiles");
                players[slot(seat)].insert(tile);
            }
        }
        // 庄家开局先摸第 14 张，各家摸齐后才换三张；这张不记 `drawn`，换牌阶段里
        // 它跟其余 13 张一样只当普通暗牌。
        let dealer_tile = wall
            .draw()
            .expect("a fresh wall always holds the dealer's opening tile");
        players[slot(dealer)].insert(dealer_tile);

        Self {
            rules,
            wall,
            dealer,
            players,
            phase: HandPhase::AwaitingExchange,
            que_suits: [None; SEATS],
            exchange_answers: [None; SEATS],
            last_discard: None,
            last_discard_was_replacement: false,
            pending_added_kan: None,
            reaction_eligible: [false; SEATS],
            reaction_answers: [None; SEATS],
            next_meld_id: 0,
            won: [false; SEATS],
            point_deltas: [0; SEATS],
            winners: Vec::new(),
            kan_events: Vec::new(),
            turns_taken: 1,
            interrupted: false,
            last_draw_was_final: false,
            last_draw_was_replacement: false,
        }
    }

    #[must_use]
    pub const fn rules(&self) -> &SichuanRules {
        &self.rules
    }

    #[must_use]
    pub const fn dealer(&self) -> Seat {
        self.dealer
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
    pub const fn que_suit(&self, seat: Seat) -> Option<Suit> {
        self.que_suits[slot(seat)]
    }

    #[must_use]
    pub const fn won(&self, seat: Seat) -> bool {
        self.won[slot(seat)]
    }

    #[must_use]
    pub fn winners(&self) -> &[WinnerRecord] {
        &self.winners
    }

    #[must_use]
    pub fn kan_events(&self) -> &[KanEvent] {
        &self.kan_events
    }

    #[must_use]
    pub const fn point_deltas(&self) -> &[i32; SEATS] {
        &self.point_deltas
    }

    #[must_use]
    pub const fn remaining_draws(&self) -> usize {
        self.wall.remaining_draws()
    }

    /// 已经实际摸走的岭上牌数量。
    #[must_use]
    pub const fn completed_rinshan_draws(&self) -> usize {
        self.wall.completed_rinshan_draws()
    }

    #[must_use]
    pub const fn exchange_direction(&self) -> ExchangeDirection {
        self.wall.exchange_direction()
    }

    #[must_use]
    pub const fn dice(&self) -> Dice {
        self.wall.dice()
    }

    #[must_use]
    pub const fn break_seat(&self) -> Seat {
        self.wall.break_seat()
    }

    #[must_use]
    pub const fn last_discard(&self) -> Option<(Seat, Tile)> {
        self.last_discard
    }

    /// 单局是否已经结束（三家胡或流局）。
    #[must_use]
    pub const fn reason(&self) -> Option<EndReason> {
        match self.phase {
            HandPhase::Ended { reason } => Some(reason),
            _ => None,
        }
    }

    /// 换三张阶段：还没选完 3 张的座位。
    #[must_use]
    pub fn pending_exchange(&self) -> Vec<Seat> {
        Seat::all()
            .into_iter()
            .filter(|seat| self.exchange_answers[slot(*seat)].is_none())
            .collect()
    }

    /// 定缺阶段：还没选定缺的座位。
    #[must_use]
    pub fn pending_dingque(&self) -> Vec<Seat> {
        Seat::all()
            .into_iter()
            .filter(|seat| self.que_suits[slot(*seat)].is_none())
            .collect()
    }

    /// 换三张：提交同花色的 3 张。全部交齐后自动对换并进入定缺。
    ///
    /// # Errors
    ///
    /// 阶段不对、已经选过、三张不同花色、或有一张不在手上。
    pub fn submit_exchange(&mut self, seat: Seat, ids: [TileId; 3]) -> Result<(), HandError> {
        if self.phase != HandPhase::AwaitingExchange {
            return self.phase_error();
        }
        if self.exchange_answers[slot(seat)].is_some() {
            return Err(HandError::UnexpectedAction);
        }
        if ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2] {
            return Err(HandError::ExchangeDuplicateTile);
        }

        let player = &self.players[slot(seat)];
        let mut kinds = Vec::with_capacity(3);
        for id in ids {
            let tile = player
                .concealed
                .iter()
                .find(|held| held.id() == id)
                .ok_or(HandError::ExchangeTileNotHeld { tile: id })?;
            kinds.push(tile.kind());
        }
        let suit = kinds[0].suit().expect("Sichuan tiles are all suited");
        if kinds[1].suit() != Some(suit) || kinds[2].suit() != Some(suit) {
            return Err(HandError::ExchangeTilesNotSameSuit);
        }

        self.exchange_answers[slot(seat)] = Some(ids);
        if self.exchange_answers.iter().all(Option::is_some) {
            self.complete_exchange();
        }
        Ok(())
    }

    /// 定缺：提交一门。全部交齐后庄家摸牌，进入打牌。
    ///
    /// # Errors
    ///
    /// 阶段不对，或已经选过。
    pub fn submit_dingque(&mut self, seat: Seat, suit: Suit) -> Result<(), HandError> {
        if self.phase != HandPhase::AwaitingDingQue {
            return self.phase_error();
        }
        if self.que_suits[slot(seat)].is_some() {
            return Err(HandError::UnexpectedAction);
        }
        self.que_suits[slot(seat)] = Some(suit);
        if self.que_suits.iter().all(Option::is_some) {
            self.start_play();
        }
        Ok(())
    }

    fn phase_error(&self) -> Result<(), HandError> {
        if self.phase.is_ended() {
            Err(HandError::HandAlreadyEnded)
        } else {
            Err(HandError::UnexpectedAction)
        }
    }

    /// 换三张：把每家选出的三张沿骰子指定方向传给接收家。
    fn complete_exchange(&mut self) {
        let direction = self.wall.exchange_direction();
        let mut selected: [Vec<Tile>; SEATS] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for seat in Seat::all() {
            let ids = self.exchange_answers[slot(seat)].expect("all seats answered");
            for id in ids {
                let tile = self.players[slot(seat)]
                    .remove_id(id)
                    .expect("exchange tiles were validated");
                selected[slot(seat)].push(tile);
            }
        }
        for source in Seat::all() {
            let recipient = direction.recipient_of(source);
            for tile in std::mem::take(&mut selected[slot(source)]) {
                self.players[slot(recipient)].insert(tile);
            }
        }
        self.exchange_answers = [None; SEATS];
        // 换三张改了各家开局手牌，天胡 / 地胡作废。
        self.interrupted = true;
        self.phase = HandPhase::AwaitingExchangeAnimation;
    }

    /// 四家换牌动画都播完后，才开放定缺选择。
    pub fn advance_from_exchange_animation(&mut self) -> Result<(), HandError> {
        if self.phase != HandPhase::AwaitingExchangeAnimation {
            return Err(HandError::UnexpectedAction);
        }
        self.phase = HandPhase::AwaitingDingQue;
        Ok(())
    }

    /// 定缺齐了：直接亮出庄家回合（第 14 张已在开局发牌时摸好）。
    fn start_play(&mut self) {
        // 庄家开局时已持 14 张牌，记下最后一张供自摸判定与听牌提示使用。
        let drawn = self.players[slot(self.dealer)]
            .concealed
            .last()
            .map(|tile| tile.id());
        self.players[slot(self.dealer)].drawn = drawn;
        self.phase = HandPhase::AwaitingTurnAction { seat: self.dealer };
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
                if player.count_of(kind) == 4 && !kinds.contains(&kind) {
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
                .filter(|meld| meld.kind() == MeldKind::Pon && player.count_of(meld.tile()) > 0)
                .map(Meld::id)
                .collect()
        } else {
            Vec::new()
        };

        TurnActions {
            can_tsumo: self.tsumo_evaluation(seat).is_some(),
            concealed_kans,
            added_kans,
            tenpai_discard_hints: self.tenpai_discard_hints(seat),
        }
    }

    #[must_use]
    pub fn reaction_options(&self, seat: Seat) -> ReactionOptions {
        let HandPhase::AwaitingResponses { discarder } = self.phase else {
            return ReactionOptions::default();
        };
        if seat == discarder || self.reaction_answers[slot(seat)].is_some() || self.won[slot(seat)]
        {
            return ReactionOptions::default();
        }
        if let Some(pending) = self.pending_added_kan {
            return self.reaction_options_for(seat, discarder, pending.tile, true);
        }
        self.last_discard
            .map_or_else(ReactionOptions::default, |(_, tile)| {
                self.reaction_options_for(seat, discarder, tile, false)
            })
    }

    fn reaction_options_for(
        &self,
        seat: Seat,
        _source: Seat,
        tile: Tile,
        chankan: bool,
    ) -> ReactionOptions {
        let kind = tile.kind();
        let player = &self.players[slot(seat)];
        let que = self.que_suits[slot(seat)];
        let gang_pao = !chankan && self.last_discard_was_replacement;
        let can_ron = self.ron_evaluation(seat, kind, chankan, gang_pao).is_some();
        if chankan {
            return ReactionOptions {
                can_ron,
                ..ReactionOptions::default()
            };
        }

        let remaining = self.wall.remaining_draws();
        // 最后一张摸完后的弃牌只能胡，不能再副露；定缺门也不能碰 / 明杠。
        let callable = remaining > 0 && kind.suit() != que;
        ReactionOptions {
            can_ron,
            can_pon: callable && player.count_of(kind) >= 2,
            can_open_kan: callable && player.count_of(kind) >= 3,
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

        let (tile, chankan) = if let Some(pending) = self.pending_added_kan {
            (pending.tile, true)
        } else {
            (
                self.last_discard.ok_or(HandError::UnexpectedAction)?.1,
                false,
            )
        };
        let options = self.reaction_options_for(seat, discarder, tile, chankan);
        match kind {
            ReactionKind::Ron if !options.can_ron => return Err(HandError::NotAWinningHand),
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

    fn discard(&mut self, seat: Seat, tile: TileId) -> Result<(), HandError> {
        let que = self.que_suits[slot(seat)];
        let player = &self.players[slot(seat)];
        let tile_kind = player
            .concealed
            .iter()
            .find(|held| held.id() == tile)
            .ok_or(HandError::TileNotHeld { tile })?
            .kind();
        // 手上有定缺门时，只能打定缺门。
        let must_discard_que = player
            .concealed
            .iter()
            .any(|held| held.kind().suit() == que);
        if must_discard_que && tile_kind.suit() != que {
            return Err(HandError::QueTilesRemaining);
        }
        let was_replacement = self.last_draw_was_replacement;

        let tile = self.players[slot(seat)].remove_id(tile)?;
        let player = &mut self.players[slot(seat)];
        player.drawn = None;
        player.discards.push(Discard::new(tile));

        self.last_draw_was_replacement = false;
        self.last_draw_was_final = false;
        self.last_discard_was_replacement = was_replacement;

        self.last_discard = Some((seat, tile));
        self.open_reactions(seat);
        Ok(())
    }

    fn tsumo(&mut self, seat: Seat) -> Result<(), HandError> {
        let evaluation = self
            .tsumo_evaluation(seat)
            .ok_or(HandError::NotAWinningHand)?;
        self.record_win(seat, evaluation, true, None, false);
        self.phase = HandPhase::AwaitingWinAnimation { seat };
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
        let que = self.que_suits[slot(seat)];
        // 胡牌时牌型不得含定缺门（含得牌）。
        if winning_tile.suit() == que {
            return None;
        }
        let concealed = player.concealed_kinds();
        if concealed.iter().any(|kind| kind.suit() == que) {
            return None;
        }
        let melds = player.meld_summaries();
        evaluate(&WinContext {
            concealed: &concealed,
            melds: &melds,
            is_tsumo: true,
            rinshan: self.last_draw_was_replacement,
            gang_pao: false,
            chankan: false,
            is_last_tile: self.last_draw_was_final,
            blessing: !self.interrupted && self.turns_taken <= BLESSING_MAX_TURNS,
        })
    }

    fn ron_evaluation(
        &self,
        seat: Seat,
        winning_tile: TileKind,
        chankan: bool,
        gang_pao: bool,
    ) -> Option<WinEvaluation> {
        let player = &self.players[slot(seat)];
        let que = self.que_suits[slot(seat)];
        if winning_tile.suit() == que {
            return None;
        }
        let mut concealed = player.concealed_kinds();
        concealed.push(winning_tile);
        if concealed.iter().any(|kind| kind.suit() == que) {
            return None;
        }
        let melds = player.meld_summaries();
        evaluate(&WinContext {
            concealed: &concealed,
            melds: &melds,
            is_tsumo: false,
            rinshan: false,
            gang_pao,
            chankan,
            is_last_tile: false,
            blessing: false,
        })
    }

    fn ron(&mut self, winner: Seat, payer: Seat, chankan: bool) {
        let tile = self.pending_added_kan.map_or_else(
            || self.last_discard.expect("ron on a discard has a tile").1,
            |pending| pending.tile,
        );
        let gang_pao = !chankan && self.last_discard_was_replacement;
        let evaluation = self
            .ron_evaluation(winner, tile.kind(), chankan, gang_pao)
            .expect("ron eligibility was checked");
        self.record_win(winner, evaluation, false, Some(payer), chankan);
        self.phase = HandPhase::AwaitingWinAnimation { seat: winner };
    }

    /// 记录一次胡并结算点数；返回 `true` 表示已经三家胡、本局结束。
    fn record_win(
        &mut self,
        winner: Seat,
        evaluation: WinEvaluation,
        is_tsumo: bool,
        payer: Option<Seat>,
        chankan: bool,
    ) -> bool {
        let score = i32::try_from(evaluation.score()).expect("capped score fits i32");
        let mut deltas = [0_i32; SEATS];
        if is_tsumo {
            for other in Seat::all() {
                if other != winner && !self.won[slot(other)] {
                    deltas[slot(winner)] += score;
                    deltas[slot(other)] -= score;
                }
            }
        } else {
            let payer = payer.expect("a ron always has a payer");
            deltas[slot(winner)] += score;
            deltas[slot(payer)] -= score;
        }
        for (index, delta) in deltas.iter().copied().enumerate() {
            self.point_deltas[index] += delta;
        }
        self.winners.push(WinnerRecord {
            seat: winner,
            evaluation,
            is_tsumo,
            payer,
            chankan,
            deltas,
        });
        self.won[slot(winner)] = true;

        self.winners.len() >= WINNERS_TO_END
    }

    /// 从 `seat` 的下家开始，跳过已胡的座位，找到下一个还能行动的座位。
    fn next_active(&self, seat: Seat) -> Seat {
        let mut next = seat.next();
        while self.won[slot(next)] {
            next = next.next();
        }
        next
    }

    fn concealed_kan(&mut self, seat: Seat, kind: TileKind) -> Result<(), HandError> {
        if self.wall.remaining_draws() == 0 {
            return Err(HandError::WallExhausted);
        }
        let tiles = self.players[slot(seat)].remove_kind(kind, 4)?;
        self.push_meld(seat, MeldKind::ConcealedKan, kind, tiles, None, None);

        let mut deltas = [0_i32; SEATS];
        for other in Seat::all() {
            if other != seat && !self.won[slot(other)] {
                deltas[slot(seat)] += CONCEALED_KAN_POINTS;
                deltas[slot(other)] -= CONCEALED_KAN_POINTS;
            }
        }
        self.apply_kan_event(seat, MeldKind::ConcealedKan, deltas);
        self.after_kan(seat)
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
        if meld.kind() != MeldKind::Pon {
            return Err(HandError::MeldNotAvailable);
        }
        let kind = meld.tile();
        let tile = player
            .concealed
            .iter()
            .find(|tile| tile.kind() == kind)
            .copied()
            .ok_or(HandError::MeldNotAvailable)?;
        self.pending_added_kan = Some(PendingAddedKan {
            declarer: seat,
            meld_id,
            tile,
        });

        self.open_chankan_reactions(seat, tile);
        if matches!(self.phase, HandPhase::AwaitingResponses { .. }) {
            return Ok(());
        }
        self.complete_added_kan();
        Ok(())
    }

    fn open_chankan_reactions(&mut self, declarer: Seat, tile: Tile) {
        self.reaction_answers = [None; SEATS];
        self.reaction_eligible = [false; SEATS];
        let mut any = false;
        for seat in Seat::all() {
            if seat == declarer || self.won[slot(seat)] {
                continue;
            }
            let eligible = self
                .reaction_options_for(seat, declarer, tile, true)
                .can_ron;
            self.reaction_eligible[slot(seat)] = eligible;
            any |= eligible;
        }
        if any {
            self.phase = HandPhase::AwaitingResponses {
                discarder: declarer,
            };
        }
    }

    fn complete_added_kan(&mut self) {
        let pending = self
            .pending_added_kan
            .take()
            .expect("an added kan is pending");
        let seat = pending.declarer;
        let meld_id = pending.meld_id;
        let tile = self.players[slot(seat)]
            .remove_id(pending.tile.id())
            .expect("the pending added-kan tile remains in hand");
        let player = &mut self.players[slot(seat)];
        let meld = player
            .melds
            .iter_mut()
            .find(|meld| meld.id() == meld_id)
            .expect("the meld was found above");
        meld.upgrade_to_added_kan(tile);
        player.kan_count += 1;
        player.drawn = None;

        let mut deltas = [0_i32; SEATS];
        for other in Seat::all() {
            if other != seat && !self.won[slot(other)] {
                deltas[slot(seat)] += ADDED_KAN_POINTS;
                deltas[slot(other)] -= ADDED_KAN_POINTS;
            }
        }
        self.apply_kan_event(seat, MeldKind::AddedKan, deltas);

        self.interrupted = true;
        self.phase = HandPhase::AwaitingKanAnimation { seat };
    }

    fn after_kan(&mut self, seat: Seat) -> Result<(), HandError> {
        self.interrupted = true;
        self.phase = HandPhase::AwaitingKanAnimation { seat };
        Ok(())
    }

    /// 杠点动画四家都播完后，由上层调用，为 `seat` 摸岭上牌并开始新回合。
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

    /// 胡牌动画四家都播完后，由上层调用。三家胡则进入结束态，否则从胡牌家的下家
    /// 开始摸牌，继续血战到底。
    pub fn advance_from_win_animation(&mut self, seat: Seat) -> Result<(), HandError> {
        match self.phase {
            HandPhase::AwaitingWinAnimation { seat: waiting } if waiting == seat => {}
            _ => return Err(HandError::UnexpectedAction),
        }
        if self.winners.len() >= WINNERS_TO_END {
            self.finish(EndReason::ThreeWinners);
        } else {
            let chankan_declarer = self
                .pending_added_kan
                .take()
                .map(|pending| pending.declarer);
            self.reset_reactions();
            if let Some(declarer) = chankan_declarer {
                self.phase = HandPhase::AwaitingDiscard { seat: declarer };
            } else {
                self.draw_and_open_turn(self.next_active(seat), DrawSource::Wall);
            }
        }
        Ok(())
    }

    fn open_reactions(&mut self, discarder: Seat) {
        self.reaction_answers = [None; SEATS];
        self.reaction_eligible = [false; SEATS];

        let tile = self
            .last_discard
            .expect("discard reactions always have a tile")
            .1;
        let mut any = false;
        for seat in Seat::all() {
            if seat == discarder || self.won[slot(seat)] {
                continue;
            }
            let eligible = !self
                .reaction_options_for(seat, discarder, tile, false)
                .is_empty();
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
                Some(ReactionKind::Ron) => {
                    chosen = Some((seat, ReactionKind::Ron));
                    break;
                }
                Some(ReactionKind::OpenKan) if chosen.is_none() => {
                    chosen = Some((seat, ReactionKind::OpenKan));
                }
                Some(ReactionKind::Pon) if chosen.is_none() => {
                    chosen = Some((seat, ReactionKind::Pon));
                }
                _ => {}
            }
        }

        let Some((seat, kind)) = chosen else {
            if self.pending_added_kan.is_some() {
                self.complete_added_kan();
            } else {
                self.advance_turn(discarder);
            }
            return;
        };
        if kind == ReactionKind::Ron {
            self.ron(seat, discarder, self.pending_added_kan.is_some());
            return;
        }
        self.execute_call(discarder, seat, kind);
    }

    fn execute_call(&mut self, discarder: Seat, caller: Seat, kind: ReactionKind) {
        let (_, called_tile) = self.last_discard.expect("a call always answers a discard");
        let tile_kind = called_tile.kind();

        if let Some(discard) = self.players[slot(discarder)].discards.last_mut() {
            discard.mark_called();
        }

        let from_hand = if kind == ReactionKind::OpenKan { 3 } else { 2 };
        let mut tiles = self.players[slot(caller)]
            .remove_kind(tile_kind, from_hand)
            .expect("eligibility was checked when the reaction was accepted");
        tiles.push(called_tile);

        let meld_kind = if kind == ReactionKind::OpenKan {
            MeldKind::OpenKan
        } else {
            MeldKind::Pon
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
        self.reset_reactions();
        self.interrupted = true;

        if meld_kind == MeldKind::OpenKan {
            let mut deltas = [0_i32; SEATS];
            deltas[slot(caller)] += OPEN_KAN_POINTS;
            deltas[slot(discarder)] -= OPEN_KAN_POINTS;
            self.apply_kan_event(caller, MeldKind::OpenKan, deltas);
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

    fn apply_kan_event(&mut self, seat: Seat, kind: MeldKind, deltas: [i32; SEATS]) {
        for (index, delta) in deltas.iter().copied().enumerate() {
            self.point_deltas[index] += delta;
        }
        self.kan_events.push(KanEvent { seat, kind, deltas });
    }

    fn advance_turn(&mut self, from: Seat) {
        self.reset_reactions();
        self.draw_and_open_turn(self.next_active(from), DrawSource::Wall);
    }

    fn reset_reactions(&mut self) {
        self.last_discard = None;
        self.reaction_eligible = [false; SEATS];
        self.reaction_answers = [None; SEATS];
    }

    fn draw_and_open_turn(&mut self, seat: Seat, source: DrawSource) {
        let drawn = match source {
            DrawSource::Wall => self.wall.draw(),
            DrawSource::Replacement => self.wall.draw_from_back().ok(),
        };
        let Some(tile) = drawn else {
            self.finish(EndReason::ExhaustiveDraw);
            return;
        };

        if source == DrawSource::Wall {
            self.turns_taken += 1;
        }
        self.last_draw_was_replacement = source == DrawSource::Replacement;
        self.last_draw_was_final = source == DrawSource::Wall && self.wall.remaining_draws() == 0;

        let player = &mut self.players[slot(seat)];
        player.insert(tile);
        player.drawn = Some(tile.id());

        self.phase = HandPhase::AwaitingTurnAction { seat };
    }

    /// 「打哪张能听」：有摸牌时，对每一种可以打出的牌种，返回打出后所有能和的牌种。
    fn tenpai_discard_hints(&self, seat: Seat) -> Vec<(TileId, Vec<TileKind>)> {
        let player = &self.players[slot(seat)];
        if player.drawn.is_none() {
            return Vec::new();
        }
        let que = self.que_suits[slot(seat)];
        let melds = player.meld_summaries();
        let all_kinds = player.concealed_kinds();
        let has_que = all_kinds.iter().any(|kind| kind.suit() == que);

        let mut seen: Vec<TileKind> = Vec::new();
        let mut hints = Vec::new();

        for tile in &player.concealed {
            let discard_kind = tile.kind();
            if seen.contains(&discard_kind) {
                continue;
            }
            seen.push(discard_kind);

            // 有定缺门时只能打定缺门。
            if has_que && discard_kind.suit() != que {
                continue;
            }

            let mut remaining = all_kinds.clone();
            let position = remaining
                .iter()
                .position(|kind| *kind == discard_kind)
                .expect("tile is in concealed hand");
            remaining.remove(position);

            let waiting: Vec<TileKind> = (0..TileKind::SUITED_KIND_COUNT)
                .map(|index| {
                    TileKind::from_index(u8::try_from(index).expect("kind index fits u8"))
                        .expect("index is in range")
                })
                .filter(|&candidate| {
                    if candidate.suit() == que {
                        return false;
                    }
                    let mut test = remaining.clone();
                    test.push(candidate);
                    if test.iter().any(|kind| kind.suit() == que) {
                        return false;
                    }
                    evaluate(&WinContext {
                        concealed: &test,
                        melds: &melds,
                        is_tsumo: false,
                        rinshan: false,
                        gang_pao: false,
                        chankan: false,
                        is_last_tile: false,
                        blessing: false,
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

    fn finish(&mut self, reason: EndReason) {
        self.last_discard = None;
        self.pending_added_kan = None;
        self.reaction_eligible = [false; SEATS];
        self.reaction_answers = [None; SEATS];
        self.phase = HandPhase::Ended { reason };
    }

    /// 手上是否还有定缺门（暗牌）。
    #[must_use]
    pub fn has_que_tiles(&self, seat: Seat) -> bool {
        let que = self.que_suits[slot(seat)];
        self.players[slot(seat)]
            .concealed
            .iter()
            .any(|tile| tile.kind().suit() == que)
    }

    /// 是否花猪：暗牌含三门（查花猪）。
    #[must_use]
    pub fn is_flower_pig(&self, seat: Seat) -> bool {
        let mut suits = [false; 3];
        for tile in self.players[slot(seat)].concealed() {
            if let Some(suit) = tile.kind().suit() {
                suits[suit.index()] = true;
            }
        }
        suits.iter().all(|present| *present)
    }

    /// 是否听牌（查大叫）。花猪或手上有定缺门都不是听牌。
    #[must_use]
    pub fn is_tenpai(&self, seat: Seat) -> bool {
        if self.has_que_tiles(seat) {
            return false;
        }
        let player = &self.players[slot(seat)];
        let que = self.que_suits[slot(seat)];
        let melds = player.meld_summaries();
        let concealed = player.concealed_kinds();

        for index in 0..TileKind::SUITED_KIND_COUNT {
            let candidate = TileKind::from_index(u8::try_from(index).expect("kind index fits u8"))
                .expect("index is in range");
            if candidate.suit() == que {
                continue;
            }
            let mut test = concealed.clone();
            test.push(candidate);
            if evaluate(&WinContext {
                concealed: &test,
                melds: &melds,
                is_tsumo: false,
                rinshan: false,
                gang_pao: false,
                chankan: false,
                is_last_tile: false,
                blessing: false,
            })
            .is_some()
            {
                return true;
            }
        }
        false
    }

    /// 开发 / 测试专用：把某个座位的暗手整体换成给定牌码。
    pub fn set_concealed_tiles(&mut self, seat: Seat, codes: &[String]) -> Result<(), HandError> {
        let player = &mut self.players[slot(seat)];
        if codes.len() != player.concealed.len() {
            return Err(HandError::WrongConcealedTileCount {
                expected: player.concealed.len(),
                actual: codes.len(),
            });
        }
        let kinds = codes
            .iter()
            .map(|code| {
                code.parse::<TileKind>()
                    .map_err(|_| HandError::InvalidTileCode(code.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (tile, kind) in player.concealed.iter_mut().zip(kinds) {
            *tile = Tile::new(tile.id(), kind);
        }
        player
            .concealed
            .sort_unstable_by_key(|tile| sort_key(*tile));
        Ok(())
    }

    /// 只给测试用：跳过换三张与定缺，直接进入打牌。
    ///
    /// 开局庄家已摸第 14 张，这里只把末张记为摸牌，供自摸 / 听牌提示使用；
    /// 摸牌次数设成超过天胡 / 地胡的阈值，这样测试里庄家自摸不会误判成天胡。
    #[cfg(test)]
    pub(crate) fn force_start_play(&mut self, que_suits: [Suit; SEATS]) {
        self.que_suits = que_suits.map(Some);
        self.phase = HandPhase::AwaitingTurnAction { seat: self.dealer };
        let drawn = self.players[slot(self.dealer)]
            .concealed
            .last()
            .map(|tile| tile.id())
            .expect("the dealer holds fourteen tiles");
        self.players[slot(self.dealer)].drawn = Some(drawn);
        self.turns_taken = BLESSING_MAX_TURNS + 1;
    }

    /// 只给测试用：直接摆成三家胡 / 流局的结束态。
    #[cfg(test)]
    pub(crate) fn force_end(&mut self, reason: EndReason) {
        self.finish(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::{SichuanHand, TurnAction};
    use crate::config::SichuanRules;
    use crate::hand::model::{EndReason, HandPhase, MeldKind};
    use crate::progress::Seat;
    use crate::scoring::Yaku;
    use crate::tile::Suit;
    use crate::wall::WallSeed;

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn seed(byte: u8) -> WallSeed {
        WallSeed::from_bytes([byte; 32])
    }

    fn opening() -> SichuanHand {
        SichuanHand::new(SichuanRules::standard(), seat(0), &seed(7))
    }

    fn que_play() -> SichuanHand {
        let mut hand = opening();
        hand.force_start_play([Suit::Man, Suit::Pin, Suit::Sou, Suit::Man]);
        hand
    }

    fn kinds(spec: &str) -> Vec<String> {
        spec.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn opening_deals_the_dealer_a_fourteenth_tile_and_awaits_exchange() {
        let hand = opening();

        assert_eq!(hand.player(seat(0)).concealed().len(), 14);
        assert!(hand.player(seat(0)).drawn().is_none());
        for index in 1..4 {
            assert_eq!(hand.player(seat(index)).concealed().len(), 13);
        }
        assert_eq!(hand.phase(), HandPhase::AwaitingExchange);
        assert_eq!(hand.pending_exchange().len(), 4);
    }

    #[test]
    fn exchange_passes_the_three_tiles_in_the_dice_direction() {
        let mut hand = opening();
        // 方向由种子决定；这里验证每家交出的牌到达该方向的接收家。给每家铺 3 张目标花色 + 10 张别门杂牌，
        // 保证每家恰好只有 3 张目标花色可以选出来。
        let direction = hand.exchange_direction();
        let suits = [Suit::Man, Suit::Pin, Suit::Sou, Suit::Man];
        let filler_suits = [Suit::Pin, Suit::Sou, Suit::Man, Suit::Pin];
        for index in 0..4 {
            let base = match suits[index] {
                Suit::Man => kinds("1m 2m 3m"),
                Suit::Pin => kinds("1p 2p 3p"),
                Suit::Sou => kinds("1s 2s 3s"),
            };
            let filler = match filler_suits[index] {
                Suit::Man => kinds("1m 1m 2m 2m 3m 3m 4m 4m 5m 5m"),
                Suit::Pin => kinds("1p 1p 2p 2p 3p 3p 4p 4p 5p 5p"),
                Suit::Sou => kinds("1s 1s 2s 2s 3s 3s 4s 4s 5s 5s"),
            };
            let mut codes = base;
            codes.extend(filler);
            if hand.dealer() == seat(index as u8) {
                // 庄家开局已摸第 14 张，补一张别门杂牌凑满 14 张。
                codes.push(
                    match filler_suits[index] {
                        Suit::Man => "6m",
                        Suit::Pin => "6p",
                        Suit::Sou => "6s",
                    }
                    .to_owned(),
                );
            }
            hand.set_concealed_tiles(seat(index as u8), &codes)
                .expect("plant hand");
        }

        // 每家选出自己那门花色的 3 张（暗牌会按种排序，不能按位置取）。
        let mut picks = [[None::<crate::tile::TileId>; 3]; 4];
        for index in 0..4 {
            let suit = suits[index];
            let ids: Vec<_> = hand
                .player(seat(index as u8))
                .concealed()
                .iter()
                .filter(|t| t.kind().suit() == Some(suit))
                .map(|t| t.id())
                .collect();
            assert_eq!(ids.len(), 3, "每家恰好 3 张指定花色");
            let ids = [ids[0], ids[1], ids[2]];
            picks[index] = ids.map(Some);
            hand.submit_exchange(seat(index as u8), ids)
                .expect("exchange valid");
        }

        assert_eq!(hand.phase(), HandPhase::AwaitingExchangeAnimation);
        hand.advance_from_exchange_animation()
            .expect("exchange animation complete");
        assert_eq!(hand.phase(), HandPhase::AwaitingDingQue);
        // 每家交出的三张都应到达该方向的接收家。
        for index in 0..4 {
            let recipient = direction.recipient_of(seat(index as u8));
            let mine = picks[index].map(Option::unwrap);
            let recipient_ids: Vec<_> = hand
                .player(recipient)
                .concealed()
                .iter()
                .map(|t| t.id())
                .collect();
            for id in mine {
                assert!(recipient_ids.contains(&id), "交出的牌到达接收家手里");
            }
        }
    }

    #[test]
    fn exchange_rejects_mixed_suits() {
        let mut hand = opening();
        // 2 张筒 + 12 张万 = 14 张（庄家），选出 2 筒 + 1 万即混花色，应拒绝。
        let codes = kinds("1p 2p 1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m");
        hand.set_concealed_tiles(seat(0), &codes)
            .expect("plant hand");

        let concealed = hand.player(seat(0)).concealed();
        let pin: Vec<_> = concealed
            .iter()
            .filter(|t| t.kind().suit() == Some(Suit::Pin))
            .map(|t| t.id())
            .collect();
        let man = concealed
            .iter()
            .find(|t| t.kind().suit() == Some(Suit::Man))
            .expect("has man")
            .id();
        let ids = [pin[0], pin[1], man];
        assert!(hand.submit_exchange(seat(0), ids).is_err());
    }

    #[test]
    fn dingque_then_the_dealer_opens_the_turn() {
        let mut hand = opening();
        // 跳过换三张：直接给每家塞一手牌然后换完。庄家开局已有第 14 张。
        for index in 0..4 {
            let codes = if hand.dealer() == seat(index) {
                kinds("1m 1m 1m 2m 2m 2m 3m 3m 3m 4m 4m 4m 5m 5m")
            } else {
                kinds("1m 1m 1m 2m 2m 2m 3m 3m 3m 4m 4m 4m 5m")
            };
            hand.set_concealed_tiles(seat(index), &codes)
                .expect("plant hand");
            let ids = [
                hand.player(seat(index)).concealed()[0].id(),
                hand.player(seat(index)).concealed()[1].id(),
                hand.player(seat(index)).concealed()[2].id(),
            ];
            hand.submit_exchange(seat(index), ids)
                .expect("exchange valid");
        }
        assert_eq!(hand.phase(), HandPhase::AwaitingExchangeAnimation);
        hand.advance_from_exchange_animation()
            .expect("exchange animation complete");

        for index in 0..4 {
            hand.submit_dingque(seat(index), Suit::Man)
                .expect("dingque valid");
        }

        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingTurnAction { seat: seat(0) }
        );
        assert_eq!(hand.player(seat(0)).concealed().len(), 14);
        // 第 14 张在开局发牌时已经摸好，start_play 会把它记到 drawn 供自摸判定使用。
        assert!(hand.player(seat(0)).drawn().is_some());
    }

    #[test]
    fn discards_must_clear_the_que_suit_first() {
        let mut hand = que_play();
        // 0 号定缺 Man，手里还留着 Man 牌，却想打 Pin → 拒绝。
        hand.set_concealed_tiles(seat(0), &kinds("1m 1m 1p 2p 3p 4p 5p 6p 7p 8p 9p 2m 3m 4m"))
            .expect("plant hand");

        let pin = hand
            .player(seat(0))
            .concealed()
            .iter()
            .find(|t| t.kind().suit() == Some(Suit::Pin))
            .expect("has pin")
            .id();
        assert!(
            hand.apply_turn_action(seat(0), TurnAction::Discard { tile: pin })
                .is_err()
        );

        let man = hand
            .player(seat(0))
            .concealed()
            .iter()
            .find(|t| t.kind().suit() == Some(Suit::Man))
            .expect("has man")
            .id();
        hand.apply_turn_action(seat(0), TurnAction::Discard { tile: man })
            .expect("discarding que suit is legal");
    }

    #[test]
    fn a_tsumo_win_marks_the_winner_and_play_continues() {
        let mut hand = que_play();
        // 0 号定缺 Man，塞一手清一色筒子自摸。0 号当前 14 张。
        let winning = kinds("1p 2p 3p 4p 5p 6p 7p 8p 9p 1p 1p 1p 2p 2p");
        hand.set_concealed_tiles(seat(0), &winning)
            .expect("plant hand");

        let actions = hand.turn_actions(seat(0));
        assert!(actions.can_tsumo);
        hand.apply_turn_action(seat(0), TurnAction::Tsumo)
            .expect("tsumo is legal");

        assert!(hand.won(seat(0)));
        assert_eq!(hand.winners().len(), 1);
        let winner = &hand.winners()[0];
        assert!(winner.is_tsumo());
        assert_eq!(winner.evaluation().fan(), 4, "清一色 3 + 自摸 1");
        assert!(
            winner
                .evaluation()
                .yaku()
                .iter()
                .any(|y| y.yaku() == Yaku::QingYiSe)
        );
        // 其余三家各付 8000 分（清一色自摸 = 8000 分），0 号得 24000 分。
        assert_eq!(hand.point_deltas(), &[24000, -8000, -8000, -8000]);
        // 先等胡牌动画握手，再由 0 号下家（1 号）行动。
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingWinAnimation { seat: seat(0) }
        );
        hand.advance_from_win_animation(seat(0))
            .expect("win animation complete");
        assert_eq!(
            hand.phase(),
            HandPhase::AwaitingTurnAction { seat: seat(1) }
        );
    }

    #[test]
    fn a_concealed_kan_charges_the_other_active_players_two_each() {
        let hand = que_play();
        // 0 号手里塞 4 张 1m 暗杠，其余任意。定缺 Sou。
        let mut hand_obj = hand;
        hand_obj
            .set_concealed_tiles(seat(0), &kinds("1m 1m 1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m"))
            .expect("plant hand");

        hand_obj
            .apply_turn_action(
                seat(0),
                TurnAction::ConcealedKan {
                    tile: "1m".parse().unwrap(),
                },
            )
            .expect("concealed kan legal");

        assert_eq!(hand_obj.point_deltas(), &[6000, -2000, -2000, -2000]);
        assert_eq!(hand_obj.kan_events().len(), 1);
        assert_eq!(hand_obj.kan_events()[0].kind(), MeldKind::ConcealedKan);
        assert_eq!(
            hand_obj.phase(),
            HandPhase::AwaitingKanAnimation { seat: seat(0) }
        );
    }

    #[test]
    fn ending_with_three_winners_reports_three_winners() {
        let mut hand = que_play();
        hand.force_end(EndReason::ThreeWinners);
        assert_eq!(hand.reason(), Some(EndReason::ThreeWinners));
    }
}
