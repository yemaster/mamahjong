use mahjong_core::{MatchId, RoomId, UserId};
use mahjong_riichi::{
    Discard, HandEvent, HandOutcome, HandPhase, HandResult, HandSettlement, MatchResult, Meld,
    MeldId, MeldKind, Reaction, RiichiHand, RiichiMatch, RiichiScorer, RiichiStatus, ScoredWinner,
    Seat, TableProgress, Tile, TileId, TileKind, WallSeed, WinEvaluation,
};

use crate::clock::SeatClock;
use crate::match_flow::{MatchOpening, SettlementFlow};
use crate::presentation::{
    animation_grace_ms, settlement_fallback_ms, settlement_reveal_fallback_ms,
};
use crate::stream::{MATCH_EVENT_PAGE_LIMIT, MatchEvent, MatchEventPage};
use crate::{ApplicationError, ErrorCode, Room, RoomLifecycle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameCommand {
    Discard {
        tile_id: u16,
    },
    RiichiDiscard {
        tile_id: u16,
    },
    Tsumo,
    Pass,
    Ron,
    Chi {
        tile_ids: [u16; 2],
    },
    Pon {
        tile_ids: [u16; 2],
    },
    OpenKan {
        tile_ids: [u16; 3],
    },
    ConcealedKan {
        tile_ids: [u16; 4],
    },
    AddedKan {
        meld_id: u8,
        tile_id: u16,
    },
    NineTerminals,
    /// 冲击麻将打牌。牌型判定全在引擎里，指令只带一张牌。
    ImpactDiscard {
        tile_id: u16,
    },
    ImpactTsumo,
    /// 冲击麻将碰。引擎自己挑手上的两张，「指示牌碰」也走这一条。
    ImpactPon,
    ImpactOpenKan,
    /// 暗杠带的是牌种（`1m` 这样的牌码），四张具体是哪四张由引擎决定。
    ImpactConcealedKan {
        tile_code: String,
    },
    ImpactAddedKan {
        meld_id: u16,
    },
    /// 手持三张指示牌的暗杠：只结算杠点，牌型仍是刻子。
    ImpactIndicatorConcealedKan,
    ImpactPass,
    /// 前端报告指定的杠点动画已播完；四家都报告后服务端才摸岭上牌。
    ImpactKanAnimationPlayed {
        kan_id: u64,
    },
    MatchAssetsReady,
    ReadyForHand {
        hand_index: u32,
    },
    SettlementPlayed {
        hand_index: u32,
    },
    ConfirmSettlement {
        hand_index: u32,
    },
    RequestExitVote,
    VoteExit {
        agree: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitGameCommand {
    pub expected_version: u64,
    pub command: GameCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchPlayer {
    user_id: UserId,
    seat: Seat,
    nickname: String,
}

impl MatchPlayer {
    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameEventRecord {
    sequence: u64,
    hand_index: u32,
    event: HandEvent,
}

impl GameEventRecord {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn hand_index(&self) -> u32 {
        self.hand_index
    }

    #[must_use]
    pub const fn event(&self) -> &HandEvent {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverPlayer {
    player: MatchPlayer,
    points: i32,
    concealed_tiles: Option<Box<[Tile]>>,
    concealed_tile_count: usize,
    drawn_tile_id: Option<TileId>,
    melds: Box<[Meld]>,
    discards: Box<[Discard]>,
    riichi_status: RiichiStatus,
    waiting_tiles: Box<[WaitingTileHint]>,
    furiten: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitingTileHint {
    code: String,
    has_yaku: bool,
}

impl WaitingTileHint {
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub const fn has_yaku(&self) -> bool {
        self.has_yaku
    }
}

impl ObserverPlayer {
    #[must_use]
    pub const fn player(&self) -> &MatchPlayer {
        &self.player
    }

    #[must_use]
    pub const fn points(&self) -> i32 {
        self.points
    }

    #[must_use]
    pub fn concealed_tiles(&self) -> Option<&[Tile]> {
        self.concealed_tiles.as_deref()
    }

    #[must_use]
    pub const fn concealed_tile_count(&self) -> usize {
        self.concealed_tile_count
    }

    #[must_use]
    pub const fn drawn_tile_id(&self) -> Option<TileId> {
        self.drawn_tile_id
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
    pub const fn riichi_status(&self) -> RiichiStatus {
        self.riichi_status
    }

    #[must_use]
    pub fn waiting_tiles(&self) -> &[WaitingTileHint] {
        &self.waiting_tiles
    }

    #[must_use]
    pub const fn is_furiten(&self) -> bool {
        self.furiten
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddedKanOption {
    meld_id: u8,
    tile_id: u16,
}

impl AddedKanOption {
    #[must_use]
    pub const fn meld_id(self) -> u8 {
        self.meld_id
    }

    #[must_use]
    pub const fn tile_id(self) -> u16 {
        self.tile_id
    }
}

/// 「打这张，就听这些牌」——立直选牌和普通听牌提示共用一种形状。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscardWaitHint {
    tile_id: u16,
    waiting_tiles: Box<[WaitingTileHint]>,
}

impl DiscardWaitHint {
    #[must_use]
    pub const fn tile_id(&self) -> u16 {
        self.tile_id
    }

    #[must_use]
    pub fn waiting_tiles(&self) -> &[WaitingTileHint] {
        &self.waiting_tiles
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnActions {
    can_tsumo: bool,
    riichi_discard_tile_ids: Box<[u16]>,
    riichi_discard_hints: Box<[DiscardWaitHint]>,
    tenpai_discard_hints: Box<[DiscardWaitHint]>,
    concealed_kan_tile_ids: Box<[[u16; 4]]>,
    added_kan_options: Box<[AddedKanOption]>,
    can_nine_terminals: bool,
}

impl TurnActions {
    #[must_use]
    pub const fn can_tsumo(&self) -> bool {
        self.can_tsumo
    }

    #[must_use]
    pub fn riichi_discard_tile_ids(&self) -> &[u16] {
        &self.riichi_discard_tile_ids
    }

    #[must_use]
    pub fn riichi_discard_hints(&self) -> &[DiscardWaitHint] {
        &self.riichi_discard_hints
    }

    /// 不管立不立直，打出哪张能听、听什么。空着就是打了也没听。
    #[must_use]
    pub fn tenpai_discard_hints(&self) -> &[DiscardWaitHint] {
        &self.tenpai_discard_hints
    }

    #[must_use]
    pub fn concealed_kan_tile_ids(&self) -> &[[u16; 4]] {
        &self.concealed_kan_tile_ids
    }

    #[must_use]
    pub fn added_kan_options(&self) -> &[AddedKanOption] {
        &self.added_kan_options
    }

    #[must_use]
    pub const fn can_nine_terminals(&self) -> bool {
        self.can_nine_terminals
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverMatch {
    id: MatchId,
    room_id: RoomId,
    observer_seat: Seat,
    version: u64,
    event_sequence: u64,
    hand_index: u32,
    progress: TableProgress,
    phase: HandPhase,
    remaining_live_draws: usize,
    dora_indicators: Box<[Tile]>,
    players: Box<[ObserverPlayer]>,
    available_reactions: Box<[Reaction]>,
    turn_actions: TurnActions,
    clocks: Box<[SeatClock]>,
    opening_ready: Box<[bool]>,
    assets_ready: Box<[bool]>,
    terminated_by_asset_timeout: bool,
    hand_settlement: Option<ObserverHandSettlement>,
    result: Option<MatchResult>,
    friend_match: bool,
    can_start_exit_vote: bool,
    exit_vote: Option<ObserverExitVote>,
    terminated_by_exit_vote: bool,
}

impl ObserverMatch {
    #[must_use]
    pub const fn id(&self) -> &MatchId {
        &self.id
    }

    #[must_use]
    pub const fn room_id(&self) -> &RoomId {
        &self.room_id
    }

    #[must_use]
    pub const fn observer_seat(&self) -> Seat {
        self.observer_seat
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    #[must_use]
    pub const fn hand_index(&self) -> u32 {
        self.hand_index
    }

    #[must_use]
    pub const fn progress(&self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn phase(&self) -> HandPhase {
        self.phase
    }

    #[must_use]
    pub const fn remaining_live_draws(&self) -> usize {
        self.remaining_live_draws
    }

    #[must_use]
    pub fn dora_indicators(&self) -> &[Tile] {
        &self.dora_indicators
    }

    #[must_use]
    pub fn players(&self) -> &[ObserverPlayer] {
        &self.players
    }

    #[must_use]
    pub fn available_reactions(&self) -> &[Reaction] {
        &self.available_reactions
    }

    #[must_use]
    pub const fn turn_actions(&self) -> &TurnActions {
        &self.turn_actions
    }

    /// Thinking time of every seat, indexed by seat.
    #[must_use]
    pub fn clocks(&self) -> &[SeatClock] {
        &self.clocks
    }

    pub fn opening_ready_seats(&self) -> impl Iterator<Item = Seat> + '_ {
        self.players
            .iter()
            .zip(self.opening_ready.iter())
            .filter(|(_, ready)| **ready)
            .map(|(player, _)| player.player().seat())
    }

    /// 已经把对局素材load完的座位。开局云雾里的「等待其他玩家」就数这个。
    pub fn assets_ready_seats(&self) -> impl Iterator<Item = Seat> + '_ {
        self.players
            .iter()
            .zip(self.assets_ready.iter())
            .filter(|(_, ready)| **ready)
            .map(|(player, _)| player.player().seat())
    }

    /// 有人迟迟没load完，这局已经作废。
    #[must_use]
    pub const fn terminated_by_asset_timeout(&self) -> bool {
        self.terminated_by_asset_timeout
    }

    #[must_use]
    pub const fn hand_settlement(&self) -> Option<&ObserverHandSettlement> {
        self.hand_settlement.as_ref()
    }

    #[must_use]
    pub const fn result(&self) -> Option<&MatchResult> {
        self.result.as_ref()
    }

    #[must_use]
    pub const fn is_friend_match(&self) -> bool {
        self.friend_match
    }

    #[must_use]
    pub const fn can_start_exit_vote(&self) -> bool {
        self.can_start_exit_vote
    }

    #[must_use]
    pub const fn exit_vote(&self) -> Option<&ObserverExitVote> {
        self.exit_vote.as_ref()
    }

    #[must_use]
    pub const fn terminated_by_exit_vote(&self) -> bool {
        self.terminated_by_exit_vote
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverExitVote {
    initiator: Seat,
    deadline_ms: u64,
    votes: Box<[Option<bool>]>,
}

impl ObserverExitVote {
    #[must_use]
    pub const fn initiator(&self) -> Seat {
        self.initiator
    }

    #[must_use]
    pub const fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }

    #[must_use]
    pub fn votes(&self) -> &[Option<bool>] {
        &self.votes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverHandSettlement {
    reason: mahjong_riichi::EndReason,
    tenpai: Box<[Seat]>,
    point_deltas: Box<[i32]>,
    points_before: Box<[i32]>,
    points_after: Box<[i32]>,
    winners: Box<[ObserverWinnerSettlement]>,
    played_seats: Box<[Seat]>,
    confirm_deadline_ms: Option<u64>,
    confirmed_seats: Box<[Seat]>,
    from: Option<Seat>,
    ura_dora_indicators: Box<[mahjong_riichi::Tile]>,
}

impl ObserverHandSettlement {
    #[must_use]
    pub const fn reason(&self) -> mahjong_riichi::EndReason {
        self.reason
    }

    #[must_use]
    pub fn tenpai(&self) -> &[Seat] {
        &self.tenpai
    }

    #[must_use]
    pub fn point_deltas(&self) -> &[i32] {
        &self.point_deltas
    }

    #[must_use]
    pub fn points_before(&self) -> &[i32] {
        &self.points_before
    }

    #[must_use]
    pub fn points_after(&self) -> &[i32] {
        &self.points_after
    }

    #[must_use]
    pub fn winners(&self) -> &[ObserverWinnerSettlement] {
        &self.winners
    }

    /// 已经报告结算动画播完的座位。
    #[must_use]
    pub fn played_seats(&self) -> &[Seat] {
        &self.played_seats
    }

    /// 确认窗口的截止时刻；`None` 表示窗口还没开，确认按钮不该出现。
    ///
    /// 这个时刻由服务端定，所有人读的是同一个秒，倒计时走完服务端自己开下一局。
    #[must_use]
    pub const fn confirm_deadline_ms(&self) -> Option<u64> {
        self.confirm_deadline_ms
    }

    #[must_use]
    pub fn confirmed_seats(&self) -> &[Seat] {
        &self.confirmed_seats
    }

    #[must_use]
    pub const fn from(&self) -> Option<Seat> {
        self.from
    }

    #[must_use]
    pub fn ura_dora_indicators(&self) -> &[mahjong_riichi::Tile] {
        &self.ura_dora_indicators
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverWinnerSettlement {
    seat: Seat,
    evaluation: WinEvaluation,
    points: u32,
    dealer: bool,
}

impl ObserverWinnerSettlement {
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn evaluation(&self) -> &WinEvaluation {
        &self.evaluation
    }

    /// Raw hand value received by the winner, excluding honba and riichi sticks.
    #[must_use]
    pub const fn points(&self) -> u32 {
        self.points
    }

    #[must_use]
    pub const fn is_dealer(&self) -> bool {
        self.dealer
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingHandSettlement {
    result: HandResult,
    flow: SettlementFlow,
}

const EXIT_VOTE_DURATION_MS: u64 = 15_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExitVote {
    initiator: Seat,
    deadline_ms: u64,
    votes: Box<[Option<bool>]>,
    /// 负数表示暂停时那家还在等动画播完，恢复后这段等待继续保留。
    paused_clock_elapsed_ms: Box<[Option<i64>]>,
}

/// 一局洗好的牌山：`tiles[..live_end]` 是活牌区，之后的十四张是王牌。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HandWall {
    pub(crate) tiles: Box<[mahjong_riichi::Tile]>,
    pub(crate) live_end: usize,
}

impl HandWall {
    fn snapshot(hand: &RiichiHand) -> Self {
        let (tiles, live_end) = hand.wall_order();
        Self {
            tiles: tiles.to_vec().into_boxed_slice(),
            live_end,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiichiRuntime {
    pub(crate) id: MatchId,
    pub(crate) room_id: RoomId,
    pub(crate) version: u64,
    pub(crate) event_sequence: u64,
    hand_index: u32,
    pub(crate) players: Box<[MatchPlayer]>,
    pub(crate) rule_snapshot: mahjong_riichi::RiichiRuleSnapshot,
    pub(crate) game: RiichiMatch,
    hand: RiichiHand,
    /// 每一局洗好的牌山，下标就是局号。牌谱要靠它画出没人摸到的那些牌。
    ///
    /// **只有对局结束之后才允许下发**，见 `MatchRecord::from_runtime`。
    pub(crate) hand_walls: Vec<HandWall>,
    /// 每一局翻出来的里宝牌指示牌，下标就是局号；流局那局是空的。
    ///
    /// 里宝牌只在这一局结算时才算得出来（要对着立直家的手牌翻），下一局一开牌山就换了，
    /// 所以得在 `finish_hand` 里当场留一份，牌谱重演的结算面板要用。
    pub(crate) hand_ura_dora: Vec<Box<[mahjong_riichi::Tile]>>,
    pub(crate) events: Vec<GameEventRecord>,
    ron_evaluations: Box<[Option<WinEvaluation>]>,
    clocks: Box<[SeatClock]>,
    opening: MatchOpening,
    pending_settlement: Option<PendingHandSettlement>,
    pub(crate) friend_match: bool,
    exit_vote_used_hand: Box<[Option<u32>]>,
    exit_vote: Option<ExitVote>,
    terminated_by_exit_vote: bool,
}

impl RiichiRuntime {
    pub(crate) fn start(room: &Room, id: MatchId, now_ms: u64) -> Result<Self, ApplicationError> {
        if room.lifecycle() != RoomLifecycle::Playing || room.active_match_id() != Some(&id) {
            return Err(internal_error("room is not linked to the starting match"));
        }
        let snapshot = room
            .rule_snapshot()
            .as_riichi()
            .ok_or_else(|| internal_error("room does not carry a riichi rule snapshot"))?;
        let rule_snapshot = snapshot.clone();
        let rules = snapshot.rules().clone();
        let thinking_time = rules.match_rules.thinking_time;
        let dealer = Seat::new(rules.variant, 0)
            .map_err(|_| internal_error("starting dealer is invalid"))?;
        let game = RiichiMatch::start(rules.clone(), dealer)
            .map_err(|error| internal_error(error.to_string()))?;
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        let (hand, transition) =
            RiichiHand::start(rules, game.progress(), game.points().iter().copied(), &seed)
                .map_err(|error| internal_error(error.to_string()))?;
        let mut members = room.members().iter().collect::<Vec<_>>();
        shuffle_players(&mut members)?;
        let players = members
            .into_iter()
            .enumerate()
            .map(|(seat_index, member)| {
                Ok(MatchPlayer {
                    user_id: member.user_id().clone(),
                    seat: Seat::new(
                        snapshot.rules().variant,
                        u8::try_from(seat_index)
                            .map_err(|_| internal_error("seat index exceeds u8"))?,
                    )
                    .map_err(|_| internal_error("room member seat is invalid"))?,
                    nickname: member.nickname().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?
            .into_boxed_slice();
        let seat_count = usize::from(snapshot.rules().variant.seat_count().value());
        let hand_walls = vec![HandWall::snapshot(&hand)];
        let mut runtime = Self {
            id,
            room_id: room.id().clone(),
            version: 1,
            event_sequence: 0,
            hand_index: 0,
            players,
            rule_snapshot,
            game,
            hand,
            hand_walls,
            hand_ura_dora: Vec::new(),
            events: Vec::new(),
            ron_evaluations: vec![None; seat_count].into_boxed_slice(),
            clocks: vec![
                SeatClock::with_limits(thinking_time.base_ms(), thinking_time.reserve_ms(),);
                seat_count
            ]
            .into_boxed_slice(),
            opening: MatchOpening::new(seat_count, now_ms),
            pending_settlement: None,
            friend_match: !room.is_matchmaking_room(),
            exit_vote_used_hand: vec![None; seat_count].into_boxed_slice(),
            exit_vote: None,
            terminated_by_exit_vote: false,
        };
        runtime.record_events(transition.into_events())?;
        runtime.rearm_clocks(now_ms)?;
        Ok(runtime)
    }

    pub(crate) fn view(&self, actor: &UserId) -> Result<ObserverMatch, ApplicationError> {
        let actor_seat = self.seat_for(actor)?;
        // 流局摊牌时把听牌者听的牌一并公开，其余情况只有本人看得到自己的听牌。
        let draw_tenpai_seats: &[Seat] = self
            .pending_settlement
            .as_ref()
            .filter(|pending| pending.result.reason() == mahjong_riichi::EndReason::ExhaustiveDraw)
            .map_or(&[], |pending| pending.result.tenpai());
        let players = self
            .players
            .iter()
            .map(|match_player| {
                let hand = self
                    .hand
                    .player(match_player.seat)
                    .map_err(|error| internal_error(error.to_string()))?;
                let concealed_tiles = (match_player.seat == actor_seat
                    || self.pending_settlement.is_some())
                .then(|| hand.concealed().to_vec().into_boxed_slice());
                let is_actor = match_player.seat == actor_seat;
                let (waiting_tiles, furiten) =
                    if is_actor || draw_tenpai_seats.contains(&match_player.seat) {
                        let waiting_tiles = self
                            .hand
                            .waiting_tile_hints(match_player.seat)
                            .map_err(|error| internal_error(error.to_string()))?
                            .iter()
                            .map(|(kind, has_yaku)| WaitingTileHint {
                                code: kind.to_string(),
                                has_yaku: *has_yaku,
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        let furiten = is_actor
                            && self
                                .hand
                                .is_furiten(actor_seat)
                                .map_err(|error| internal_error(error.to_string()))?;
                        (waiting_tiles, furiten)
                    } else {
                        (Vec::<WaitingTileHint>::new().into_boxed_slice(), false)
                    };
                Ok(ObserverPlayer {
                    player: match_player.clone(),
                    points: if self.pending_settlement.is_some() {
                        hand.points()
                    } else {
                        self.game.result().map_or(hand.points(), |result| {
                            result.final_points()[usize::from(match_player.seat.index())]
                        })
                    },
                    concealed_tile_count: hand.concealed().len(),
                    concealed_tiles,
                    drawn_tile_id: (match_player.seat == actor_seat)
                        .then_some(hand.drawn_tile_id())
                        .flatten(),
                    melds: hand.melds().to_vec().into_boxed_slice(),
                    discards: hand.discards().to_vec().into_boxed_slice(),
                    riichi_status: hand.riichi_status(),
                    waiting_tiles,
                    furiten,
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?
            .into_boxed_slice();
        Ok(ObserverMatch {
            id: self.id.clone(),
            room_id: self.room_id.clone(),
            observer_seat: actor_seat,
            version: self.version,
            event_sequence: self.event_sequence,
            hand_index: self.hand_index,
            progress: self.hand.progress(),
            phase: self.hand.phase(),
            remaining_live_draws: self.hand.remaining_live_draws(),
            dora_indicators: self
                .hand
                .current_dora_indicators()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            players,
            available_reactions: if self.exit_vote.is_some()
                || self.terminated_by_exit_vote
                || self.assets_loading()
                || self.opening.terminated_by_asset_timeout()
            {
                Box::new([])
            } else {
                self.hand
                    .available_reactions(actor_seat, &RiichiScorer)
                    .map_err(|error| internal_error(error.to_string()))?
                    .into_boxed_slice()
            },
            turn_actions: self.available_turn_actions(actor_seat)?,
            clocks: self.clocks.clone(),
            opening_ready: self.opening.opening_ready_flags().into(),
            assets_ready: self.opening.assets_ready_flags().into(),
            terminated_by_asset_timeout: self.opening.terminated_by_asset_timeout(),
            hand_settlement: self.pending_settlement.as_ref().map(|pending| {
                ObserverHandSettlement {
                    reason: pending.result.reason(),
                    tenpai: pending.result.tenpai().to_vec().into_boxed_slice(),
                    point_deltas: pending.result.point_deltas().to_vec().into_boxed_slice(),
                    points_before: pending.result.points_before().to_vec().into_boxed_slice(),
                    points_after: pending.result.points_after().to_vec().into_boxed_slice(),
                    winners: pending
                        .result
                        .winners()
                        .iter()
                        .map(|winner| {
                            let seat_count = self.game.rules().variant.seat_count().value();
                            let is_dealer = winner.seat() == pending.result.progress().dealer();
                            ObserverWinnerSettlement {
                                seat: winner.seat(),
                                points: winner
                                    .evaluation()
                                    .payment()
                                    .total_received(seat_count, is_dealer),
                                dealer: is_dealer,
                                evaluation: winner.evaluation().clone(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    played_seats: self.seats_with_flag(pending.flow.played_flags()),
                    confirm_deadline_ms: pending.flow.confirm_deadline_ms(),
                    confirmed_seats: self.seats_with_flag(pending.flow.confirmed_flags()),
                    from: pending.result.from(),
                    /* 和牌谱读的是同一份，见 `finish_hand` 里存进去的那一步。 */
                    ura_dora_indicators: self.hand_ura_dora.last().cloned().unwrap_or_default(),
                }
            }),
            result: self.game.result().cloned(),
            friend_match: self.friend_match,
            can_start_exit_vote: self.friend_match
                && !self.assets_loading()
                && !self.opening.terminated_by_asset_timeout()
                && self.exit_vote.is_none()
                && self.exit_vote_used_hand[usize::from(actor_seat.index())]
                    != Some(self.hand_index)
                && !self.terminated_by_exit_vote
                && self.pending_settlement.is_none()
                && self.game.result().is_none(),
            exit_vote: self.exit_vote.as_ref().map(|vote| ObserverExitVote {
                initiator: vote.initiator,
                deadline_ms: vote.deadline_ms,
                votes: vote.votes.clone(),
            }),
            terminated_by_exit_vote: self.terminated_by_exit_vote,
        })
    }

    /// 还有人没把对局素材load完。这期间桌面是冻着的。
    fn assets_loading(&self) -> bool {
        self.opening.assets_loading()
    }

    fn available_turn_actions(&self, actor: Seat) -> Result<TurnActions, ApplicationError> {
        if self.exit_vote.is_some()
            || self.terminated_by_exit_vote
            || self.assets_loading()
            || self.opening.terminated_by_asset_timeout()
        {
            return Ok(TurnActions::default());
        }
        /*
         * 鸣完牌只等着打一张的那段是 AwaitingDiscard：立直、杠、自摸这些都不能选，
         * 但手上仍然是自己在挑打哪张，试打看听的提示照样得给。
         */
        let awaiting_discard =
            matches!(self.hand.phase(), HandPhase::AwaitingDiscard { seat } if seat == actor);
        if !awaiting_discard
            && !matches!(self.hand.phase(), HandPhase::AwaitingTurnAction { seat } if seat == actor)
        {
            return Ok(TurnActions::default());
        }
        let player = self
            .hand
            .player(actor)
            .map_err(|error| internal_error(error.to_string()))?;

        /* 单纯试打一张看听不听，副露过的手也算，跟能不能立直无关。 */
        let tenpai_discard_hints = player
            .concealed()
            .iter()
            .filter_map(|tile| {
                let mut hand = self.hand.clone();
                hand.discard(actor, tile.id()).ok()?;
                let waiting_tiles = hand
                    .waiting_tile_hints(actor)
                    .ok()?
                    .iter()
                    .map(|(kind, has_yaku)| WaitingTileHint {
                        code: kind.to_string(),
                        has_yaku: *has_yaku,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                (!waiting_tiles.is_empty()).then(|| DiscardWaitHint {
                    tile_id: tile.id().value(),
                    waiting_tiles,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        if awaiting_discard {
            return Ok(TurnActions {
                tenpai_discard_hints,
                ..TurnActions::default()
            });
        }

        let riichi_discard_hints = player
            .concealed()
            .iter()
            .filter_map(|tile| {
                let mut hand = self.hand.clone();
                hand.declare_riichi_and_discard(actor, tile.id(), &RiichiScorer)
                    .ok()?;
                let waiting_tiles = hand
                    .waiting_tile_hints(actor)
                    .ok()?
                    .iter()
                    .map(|(kind, _)| WaitingTileHint {
                        code: kind.to_string(),
                        has_yaku: true,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Some(DiscardWaitHint {
                    tile_id: tile.id().value(),
                    waiting_tiles,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let riichi_discard_tile_ids = riichi_discard_hints
            .iter()
            .map(DiscardWaitHint::tile_id)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let mut tiles_by_kind = vec![Vec::new(); TileKind::COUNT];
        for tile in player.concealed() {
            tiles_by_kind[tile.kind().index()].push(tile.id());
        }
        let concealed_kan_tile_ids = tiles_by_kind
            .into_iter()
            .filter(|tiles| tiles.len() == 4)
            .filter_map(|tiles| {
                let tile_ids: [TileId; 4] = tiles.try_into().ok()?;
                let mut hand = self.hand.clone();
                hand.declare_concealed_kan(actor, tile_ids, &RiichiScorer)
                    .is_ok()
                    .then_some(tile_ids.map(TileId::value))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let mut added_kan_options = Vec::new();
        for meld in player
            .melds()
            .iter()
            .filter(|meld| matches!(meld.kind(), MeldKind::Pon))
        {
            for tile in player
                .concealed()
                .iter()
                .filter(|tile| tile.kind() == meld.tile_kind())
            {
                let mut hand = self.hand.clone();
                if hand.declare_added_kan(actor, meld.id(), tile.id()).is_ok() {
                    added_kan_options.push(AddedKanOption {
                        meld_id: meld.id().value(),
                        tile_id: tile.id().value(),
                    });
                }
            }
        }

        let mut nine_terminals_hand = self.hand.clone();
        Ok(TurnActions {
            can_tsumo: self.hand.evaluate_tsumo(actor).is_ok(),
            riichi_discard_tile_ids,
            riichi_discard_hints,
            tenpai_discard_hints,
            concealed_kan_tile_ids,
            added_kan_options: added_kan_options.into_boxed_slice(),
            can_nine_terminals: nine_terminals_hand.declare_nine_terminals(actor).is_ok(),
        })
    }

    /// 把一份「每个座位一个开关」的标记数组翻成座位表。
    fn seats_with_flag(&self, flags: &[bool]) -> Box<[Seat]> {
        flags
            .iter()
            .enumerate()
            .filter(|(_, set)| **set)
            .map(|(index, _)| {
                Seat::new(
                    self.game.rules().variant,
                    u8::try_from(index).expect("seat count fits u8"),
                )
                .expect("seat index is valid")
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    pub(crate) fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        let opening_hand_index = match command.command {
            GameCommand::ReadyForHand { hand_index } => Some(hand_index),
            _ => None,
        };
        let played_hand_index = match command.command {
            GameCommand::SettlementPlayed { hand_index } => Some(hand_index),
            _ => None,
        };
        let settlement_hand_index = match command.command {
            GameCommand::ConfirmSettlement { hand_index } => Some(hand_index),
            _ => None,
        };
        let assets_ready_command = matches!(command.command, GameCommand::MatchAssetsReady);
        if opening_hand_index.is_none()
            && played_hand_index.is_none()
            && settlement_hand_index.is_none()
            && !assets_ready_command
            && command.expected_version != self.version
        {
            return Err(ApplicationError::new(
                ErrorCode::MatchVersionConflict,
                format!(
                    "expected match version {}, current version is {}",
                    command.expected_version, self.version
                ),
            ));
        }
        let seat = self.seat_for(actor)?;
        if self.opening.terminated_by_asset_timeout() {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match was terminated while waiting for players to load",
            ));
        }
        if assets_ready_command {
            let report = self
                .opening
                .report_assets_ready(usize::from(seat.index()), now_ms);
            if !report.changed() {
                return Ok(());
            }
            if report.everyone_ready() {
                // 最后一家也load完了，开局动画从这一刻起算，牌局到这里才真的开始。
                self.rearm_clocks(now_ms)?;
            }
            self.version = self
                .version
                .checked_add(1)
                .ok_or_else(|| internal_error("match version overflow"))?;
            return Ok(());
        }
        // 还有人没load完就一步都不许走：开局摸牌、退出投票、报告动画播完，全挡。
        if self.assets_loading() {
            return Err(invalid_command("players are still loading match assets"));
        }
        match &command.command {
            GameCommand::RequestExitVote => {
                self.request_exit_vote(seat, now_ms)?;
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or_else(|| internal_error("match version overflow"))?;
                return Ok(());
            }
            GameCommand::VoteExit { agree } => {
                self.cast_exit_vote(seat, *agree, now_ms)?;
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or_else(|| internal_error("match version overflow"))?;
                return Ok(());
            }
            _ if self.exit_vote.is_some() => {
                return Err(invalid_command("the match is paused for an exit vote"));
            }
            _ => {}
        }
        if let Some(hand_index) = opening_hand_index {
            if hand_index != self.hand_index {
                return Err(invalid_command(
                    "the opening animation is no longer current",
                ));
            }
            let report = self
                .opening
                .report_opening_ready(usize::from(seat.index()), now_ms);
            if !report.changed() {
                return Ok(());
            }
            if report.everyone_ready() {
                self.rearm_clocks(now_ms)?;
            }
            self.version = self
                .version
                .checked_add(1)
                .ok_or_else(|| internal_error("match version overflow"))?;
            return Ok(());
        }
        if let Some(hand_index) = played_hand_index {
            if hand_index != self.hand_index {
                return Err(invalid_command("the hand settlement is no longer current"));
            }
            let pending = self
                .pending_settlement
                .as_mut()
                .ok_or_else(|| invalid_command("there is no hand settlement being played"))?;
            let report = pending
                .flow
                .report_played(usize::from(seat.index()), now_ms);
            if !report.changed() {
                return Ok(());
            }
            self.version = self
                .version
                .checked_add(1)
                .ok_or_else(|| internal_error("match version overflow"))?;
            return Ok(());
        }
        if let Some(hand_index) = settlement_hand_index {
            if hand_index != self.hand_index {
                return Err(invalid_command("the hand settlement is no longer current"));
            }
            let pending = self
                .pending_settlement
                .as_mut()
                .ok_or_else(|| invalid_command("there is no hand settlement to confirm"))?;
            if !pending.flow.confirmation_open() {
                // 还有人在播结算动画，确认窗口没开，按钮也就还没下发。
                return Err(invalid_command("the settlement is still being played"));
            }
            let report = pending.flow.report_confirmed(usize::from(seat.index()));
            if !report.changed() {
                return Ok(());
            }
            if report.everyone_ready() {
                self.advance_settlement(now_ms)?;
                self.rearm_clocks(now_ms)?;
            }
            self.version = self
                .version
                .checked_add(1)
                .ok_or_else(|| internal_error("match version overflow"))?;
            return Ok(());
        }
        if self.game.result().is_some() {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match is already finished",
            ));
        }
        // 开局的牌还在往各家手上飞，谁都还没看清自己的牌，这时候谁也不许动手。
        // 有一家播完了没多久就会全场放行（见 opening_ready_deadline_passed），
        // 所以这里拒绝的窗口很短，客户端重试一次就过了。
        if self.opening.opening_blocked() {
            return Err(invalid_command("the opening deal is still being dealt"));
        }
        // 这一步在客户端要播多久的动画，决定下一个决策者晚多久开始读秒。
        let grace_ms = animation_grace_ms(&command.command);
        let mut tsumo_evaluation = None;
        let transition = match command.command {
            GameCommand::Discard { tile_id } => self.hand.discard(seat, TileId::new(tile_id)),
            GameCommand::RiichiDiscard { tile_id } => {
                self.hand
                    .declare_riichi_and_discard(seat, TileId::new(tile_id), &RiichiScorer)
            }
            GameCommand::Tsumo => {
                tsumo_evaluation = Some(self.hand.evaluate_tsumo(seat).map_err(invalid_command)?);
                self.hand.declare_tsumo(seat, &RiichiScorer)
            }
            GameCommand::Pass => self.hand.pass(seat, &RiichiScorer),
            GameCommand::Ron => {
                let evaluation = self
                    .hand
                    .evaluate_pending_ron(seat)
                    .map_err(invalid_command)?;
                let transition = self.hand.respond(seat, Reaction::Ron, &RiichiScorer);
                if transition.is_ok() {
                    self.ron_evaluations[usize::from(seat.index())] = Some(evaluation);
                }
                transition
            }
            GameCommand::Chi { tile_ids } => self.hand.respond(
                seat,
                Reaction::Chi {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::Pon { tile_ids } => self.hand.respond(
                seat,
                Reaction::Pon {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::OpenKan { tile_ids } => self.hand.respond(
                seat,
                Reaction::OpenKan {
                    hand_tiles: tile_ids.map(TileId::new),
                },
                &RiichiScorer,
            ),
            GameCommand::ConcealedKan { tile_ids } => {
                self.hand
                    .declare_concealed_kan(seat, tile_ids.map(TileId::new), &RiichiScorer)
            }
            GameCommand::AddedKan { meld_id, tile_id } => {
                self.hand
                    .declare_added_kan(seat, MeldId::new(meld_id), TileId::new(tile_id))
            }
            GameCommand::NineTerminals => self.hand.declare_nine_terminals(seat),
            GameCommand::ImpactDiscard { .. }
            | GameCommand::ImpactTsumo
            | GameCommand::ImpactPon
            | GameCommand::ImpactOpenKan
            | GameCommand::ImpactConcealedKan { .. }
            | GameCommand::ImpactAddedKan { .. }
            | GameCommand::ImpactIndicatorConcealedKan
            | GameCommand::ImpactPass
            | GameCommand::ImpactKanAnimationPlayed { .. } => {
                return Err(invalid_command(
                    "this command is not part of riichi mahjong",
                ));
            }
            GameCommand::MatchAssetsReady | GameCommand::ReadyForHand { .. } => {
                unreachable!("handled before hand command")
            }
            GameCommand::SettlementPlayed { .. } | GameCommand::ConfirmSettlement { .. } => {
                unreachable!("handled before hand command")
            }
            GameCommand::RequestExitVote | GameCommand::VoteExit { .. } => {
                unreachable!("handled before hand command")
            }
        }
        .map_err(invalid_command)?;
        let automatic = self
            .hand
            .advance_automatic_reactions(&RiichiScorer)
            .map_err(invalid_command)?;
        let mut events = transition.into_events().into_vec();
        events.extend(automatic.into_events());
        let events = events.into_boxed_slice();
        self.synchronize_riichi(&events)?;
        let ended = events.iter().find(|event| {
            matches!(
                event,
                HandEvent::TsumoDeclared { .. }
                    | HandEvent::RonDeclared { .. }
                    | HandEvent::AbortiveDrawDeclared { .. }
                    | HandEvent::ExhaustiveDrawDeclared { .. }
            )
        });
        let outcome = ended
            .map(|event| self.outcome_from_event(event, tsumo_evaluation))
            .transpose()?;
        self.record_events(events)?;
        if let Some(outcome) = outcome {
            self.finish_hand(outcome, now_ms)?;
        }
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        self.clocks[usize::from(seat.index())].disarm(now_ms);
        self.rearm_clocks_after(now_ms, grace_ms)?;
        Ok(())
    }

    fn request_exit_vote(&mut self, seat: Seat, now_ms: u64) -> Result<(), ApplicationError> {
        if !self.friend_match {
            return Err(invalid_command(
                "exit votes are only available in friend matches",
            ));
        }
        if self.terminated_by_exit_vote || self.game.result().is_some() {
            return Err(invalid_command("match is already finished"));
        }
        if self.pending_settlement.is_some() {
            return Err(invalid_command(
                "exit vote cannot start during hand settlement",
            ));
        }
        if self.exit_vote.is_some() {
            return Err(invalid_command("an exit vote is already active"));
        }
        let seat_index = usize::from(seat.index());
        if self.exit_vote_used_hand[seat_index] == Some(self.hand_index) {
            return Err(invalid_command(
                "this player already started an exit vote in this hand",
            ));
        }
        self.exit_vote_used_hand[seat_index] = Some(self.hand_index);
        let mut votes = vec![None; self.players.len()].into_boxed_slice();
        votes[seat_index] = Some(true);
        let paused_clock_elapsed_ms = self
            .clocks
            .iter_mut()
            .map(|clock| clock.pause(now_ms))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.exit_vote = Some(ExitVote {
            initiator: seat,
            deadline_ms: now_ms.saturating_add(EXIT_VOTE_DURATION_MS),
            votes,
            paused_clock_elapsed_ms,
        });
        Ok(())
    }

    fn cast_exit_vote(
        &mut self,
        seat: Seat,
        agree: bool,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        let vote = self
            .exit_vote
            .as_mut()
            .ok_or_else(|| invalid_command("there is no active exit vote"))?;
        let choice = &mut vote.votes[usize::from(seat.index())];
        if choice.is_some() {
            return Err(invalid_command("this player already voted"));
        }
        *choice = Some(agree);
        self.resolve_exit_vote(now_ms, false)
    }

    fn advance_exit_vote_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<Option<UserId>, ApplicationError> {
        let Some(vote) = self.exit_vote.as_ref() else {
            return Ok(None);
        };
        if now_ms < vote.deadline_ms {
            return Ok(None);
        }
        let actor = self
            .players
            .iter()
            .find(|player| player.seat == vote.initiator)
            .map(|player| player.user_id.clone())
            .ok_or_else(|| internal_error("exit vote initiator is missing"))?;
        self.resolve_exit_vote(now_ms, true)?;
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        Ok(Some(actor))
    }

    fn resolve_exit_vote(
        &mut self,
        now_ms: u64,
        apply_default_agreement: bool,
    ) -> Result<(), ApplicationError> {
        let Some(vote) = self.exit_vote.as_mut() else {
            return Ok(());
        };
        if apply_default_agreement {
            for choice in &mut vote.votes {
                if choice.is_none() {
                    *choice = Some(true);
                }
            }
        }
        let required = self.players.len().div_ceil(2);
        let agrees = vote
            .votes
            .iter()
            .filter(|choice| **choice == Some(true))
            .count();
        if agrees >= required {
            self.exit_vote = None;
            self.terminated_by_exit_vote = true;
            for clock in &mut self.clocks {
                clock.pause(now_ms);
            }
            return Ok(());
        }
        if vote.votes.iter().any(Option::is_none) {
            return Ok(());
        }
        let rejected = self
            .exit_vote
            .take()
            .ok_or_else(|| internal_error("exit vote disappeared"))?;
        for (clock, elapsed) in self
            .clocks
            .iter_mut()
            .zip(rejected.paused_clock_elapsed_ms.iter().copied())
        {
            clock.resume(now_ms, elapsed);
        }
        Ok(())
    }

    /// Puts every seat that owes a decision on the clock and settles the rest.
    fn rearm_clocks(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        self.rearm_clocks_after(now_ms, 0)
    }

    /// Same, but the seats that just started waiting only begin counting once
    /// the previous action has finished animating on the clients.
    ///
    /// While the tile is still flying to the river or the meld is still being
    /// pushed out, nobody can even see the position they are supposed to react
    /// to, so that stretch must not come out of anybody's thinking time. A seat
    /// that is *already* on the clock keeps its original start time — the grace
    /// belongs to the new decision, not to one that was running all along.
    fn rearm_clocks_after(&mut self, now_ms: u64, grace_ms: u64) -> Result<(), ApplicationError> {
        if self.exit_vote.is_some() || self.terminated_by_exit_vote {
            return Ok(());
        }
        if self.assets_loading()
            || self.opening.terminated_by_asset_timeout()
            || self.opening.opening_blocked()
        {
            for clock in &mut self.clocks {
                clock.disarm(now_ms);
            }
            return Ok(());
        }
        let seats = self
            .players
            .iter()
            .map(|player| player.seat)
            .collect::<Vec<_>>();
        let start_ms = now_ms.saturating_add(grace_ms);
        for seat in seats {
            let index = usize::from(seat.index());
            if self.is_waiting(seat)? {
                self.clocks[index].arm(start_ms);
            } else {
                self.clocks[index].disarm(now_ms);
            }
        }
        Ok(())
    }

    /// Whether the hand cannot advance until this seat decides.
    fn is_waiting(&self, seat: Seat) -> Result<bool, ApplicationError> {
        Ok(match self.hand.phase() {
            HandPhase::AwaitingTurnAction { seat: waiting }
            | HandPhase::AwaitingDiscard { seat: waiting } => waiting == seat,
            HandPhase::AwaitingResponses { .. } => !self
                .hand
                .available_reactions(seat, &RiichiScorer)
                .map_err(|error| internal_error(error.to_string()))?
                .is_empty(),
            HandPhase::Ended { .. } => false,
        })
    }

    /// The most conservative action for a seat that ran out of time.
    ///
    /// Discards the drawn tile when there is one, otherwise the rightmost
    /// concealed tile that the rules accept. Never wins, calls or declares
    /// riichi on the player's behalf.
    fn timeout_command(&self, seat: Seat) -> Result<GameCommand, ApplicationError> {
        if matches!(self.hand.phase(), HandPhase::AwaitingResponses { .. }) {
            return Ok(GameCommand::Pass);
        }
        let player = self
            .hand
            .player(seat)
            .map_err(|error| internal_error(error.to_string()))?;
        let candidates = player
            .drawn_tile_id()
            .into_iter()
            .chain(player.concealed().iter().rev().map(|tile| tile.id()));
        for tile_id in candidates {
            let mut hand = self.hand.clone();
            if hand.discard(seat, tile_id).is_ok() {
                return Ok(GameCommand::Discard {
                    tile_id: tile_id.value(),
                });
            }
        }
        Err(internal_error("seat has no legal discard"))
    }

    /// Advances at most one expired seat; returns the seat it acted for.
    pub(crate) fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        if let Some(actor) = self.advance_exit_vote_if_due(now_ms)? {
            return Ok(Some(actor));
        }
        if self.is_finished() {
            return Ok(None);
        }
        if self.assets_loading() {
            // 素材还没load完，谁的时钟都没上弦；等不及的那家走
            // terminate_if_assets_stalled 直接作废整局。
            return Ok(None);
        }
        if self.opening.opening_blocked() {
            // 放行走 release_opening_if_due，那条路会把新版本广播出去；客户端
            // 一直等着「全场都播完了」，没有广播就会一直等下去。
            return Ok(None);
        }
        let Some(seat) = self
            .players
            .iter()
            .map(|player| player.seat)
            .find(|seat| self.clocks[usize::from(seat.index())].expired(now_ms))
        else {
            return Ok(None);
        };
        let actor = self
            .players
            .iter()
            .find(|player| player.seat == seat)
            .map(|player| player.user_id.clone())
            .ok_or_else(|| internal_error("expired seat has no player"))?;
        let command = SubmitGameCommand {
            expected_version: self.version,
            command: self.timeout_command(seat)?,
        };
        self.execute(&actor, command, now_ms).inspect_err(|_| {
            // Keep a rejected timeout from being retried every sweep.
            self.clocks[usize::from(seat.index())].disarm(now_ms);
        })?;
        Ok(Some(actor))
    }

    fn synchronize_riichi(&mut self, events: &[HandEvent]) -> Result<(), ApplicationError> {
        for event in events {
            if let HandEvent::RiichiEstablished {
                seat,
                points_after,
                riichi_sticks,
            } = event
            {
                self.game
                    .record_riichi_established(*seat, *points_after, *riichi_sticks)
                    .map_err(|error| internal_error(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn outcome_from_event(
        &self,
        event: &HandEvent,
        tsumo_evaluation: Option<WinEvaluation>,
    ) -> Result<HandOutcome, ApplicationError> {
        match event {
            HandEvent::TsumoDeclared { winner, .. } => Ok(HandOutcome::Tsumo {
                winner: ScoredWinner::new(
                    *winner,
                    tsumo_evaluation
                        .ok_or_else(|| internal_error("tsumo evaluation is missing"))?,
                ),
            }),
            HandEvent::RonDeclared { winners, from, .. } => Ok(HandOutcome::Ron {
                from: *from,
                winners: winners
                    .iter()
                    .map(|winner| {
                        self.ron_evaluations[usize::from(winner.index())]
                            .clone()
                            .map(|evaluation| ScoredWinner::new(*winner, evaluation))
                            .ok_or_else(|| internal_error("ron evaluation is missing"))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            }),
            HandEvent::ExhaustiveDrawDeclared { tenpai, .. } => {
                let nagashi_winners = if self.game.rules().scoring.nagashi_mangan {
                    self.players
                        .iter()
                        .filter_map(|player| {
                            let hand = self.hand.player(player.seat).ok()?;
                            RiichiScorer.is_nagashi_mangan(hand).then_some(player.seat)
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                } else {
                    Box::new([])
                };
                Ok(HandOutcome::ExhaustiveDraw {
                    tenpai: if nagashi_winners.is_empty() {
                        tenpai.clone()
                    } else {
                        Box::new([])
                    },
                    nagashi_winners,
                })
            }
            HandEvent::AbortiveDrawDeclared { reason, .. } => {
                Ok(HandOutcome::AbortiveDraw { reason: *reason })
            }
            _ => Err(internal_error("event does not finish a hand")),
        }
    }

    fn finish_hand(&mut self, outcome: HandOutcome, now_ms: u64) -> Result<(), ApplicationError> {
        let result = HandSettlement
            .settle(
                self.game.rules(),
                self.game.progress(),
                self.game.points().iter().copied(),
                outcome,
            )
            .map_err(|error| internal_error(error.to_string()))?;
        self.game
            .apply_hand(result.clone())
            .map_err(|error| internal_error(error.to_string()))?;
        /*
         * 里宝牌当场留一份：下一局一开牌山就换了，之后再也算不回来。牌谱重演的结算
         * 面板要用它，实时对局的结算视图也读的是这一份，两边不可能对不上。
         * 流局不翻里宝牌，所以没人和牌就存空的。
         */
        self.hand_ura_dora.push(if result.winners().is_empty() {
            Box::new([])
        } else {
            self.hand.matching_ura_dora_indicators().collect()
        });
        self.pending_settlement = Some(PendingHandSettlement {
            result,
            flow: SettlementFlow::new(self.players.len(), now_ms),
        });
        self.ron_evaluations.fill(None);
        Ok(())
    }

    fn advance_settlement(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        self.pending_settlement
            .take()
            .ok_or_else(|| invalid_command("there is no hand settlement to advance"))?;
        if self.game.result().is_some() {
            return Ok(());
        }
        self.hand_index = u32::try_from(self.game.hands().len())
            .map_err(|_| internal_error("hand index overflow"))?;
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        let (hand, transition) = RiichiHand::start(
            self.game.rules().clone(),
            self.game.progress(),
            self.game.points().iter().copied(),
            &seed,
        )
        .map_err(|error| internal_error(error.to_string()))?;
        self.hand = hand;
        self.hand_walls.push(HandWall::snapshot(&self.hand));
        let thinking_time = self.game.rules().match_rules.thinking_time;
        self.clocks.fill(SeatClock::with_limits(
            thinking_time.base_ms(),
            thinking_time.reserve_ms(),
        ));
        self.opening.reset_hand(now_ms);
        self.record_events(transition.into_events())
    }

    /// 结算动画都播完了就开确认窗口，返回这一次是否真的开了。
    ///
    /// 窗口一开，五秒倒计时对全场同时起算：截止时刻写在视图里，各家读的是同一个
    /// 数。和放行开局一样，这一步必须走 `expire_clocks` 才能广播出去——客户端在
    /// 收到窗口之前不显示确认按钮，服务端不出声它就一直等。
    pub(crate) fn open_settlement_confirm_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.exit_vote.is_some() || self.terminated_by_exit_vote {
            return Ok(false);
        }
        let Some(pending) = self.pending_settlement.as_mut() else {
            return Ok(false);
        };
        // 确认窗口只在「全场都报告播完」或「兜底到期」两个时刻打开：
        // －全场报告 → execute() 里直接设 confirm_started_at_ms
        // －兜底到期 → 按役种条数动态计算：番种越多播报越长
        // 不设短宽限：结算动画时长随役种多少波动很大，各家播完的时刻并不同步，
        // 一家早到不该替全场抢跑。
        let yaku_count: usize = pending
            .result
            .winners()
            .iter()
            .map(|w| w.evaluation().yaku().len())
            .sum();
        if !pending
            .flow
            .open_confirmation_if_due(now_ms, settlement_reveal_fallback_ms(yaku_count))
        {
            return Ok(false);
        }
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        Ok(true)
    }

    /// 确认倒计时走完就开下一局，谁都不点也照开。
    pub(crate) fn advance_settlement_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.exit_vote.is_some() || self.terminated_by_exit_vote {
            return Ok(false);
        }
        let due = self.pending_settlement.as_ref().is_some_and(|pending| {
            // 窗口本身也有兜底：万一没人上报也没人点，整段流程仍然有个尽头。
            let yaku_count: usize = pending
                .result
                .winners()
                .iter()
                .map(|w| w.evaluation().yaku().len())
                .sum();
            pending
                .flow
                .advance_due(now_ms, settlement_fallback_ms(yaku_count))
        });
        if !due {
            return Ok(false);
        }
        self.advance_settlement(now_ms)?;
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    /// 开局动画的宽限到期时替没报告的人补上，返回这一次是否真的放行了。
    ///
    /// 单独一个入口是因为放行必须广播出去：客户端在看到「全场都播完了」之前
    /// 一直按着不让点牌，服务端不出声它就一直等下去。
    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        if self.is_finished() {
            return Ok(false);
        }
        if self.assets_loading() {
            // 开局动画的计时从「全场都load完」那一刻才起算。
            return Ok(false);
        }
        if !self.opening.release_opening_if_due(now_ms) {
            return Ok(false);
        }
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    fn record_events(&mut self, events: Box<[HandEvent]>) -> Result<(), ApplicationError> {
        self.events.reserve(events.len());
        for event in events {
            self.event_sequence = self
                .event_sequence
                .checked_add(1)
                .ok_or_else(|| internal_error("event sequence overflow"))?;
            self.events.push(GameEventRecord {
                sequence: self.event_sequence,
                hand_index: self.hand_index,
                event,
            });
        }
        Ok(())
    }

    /// Reads events after `after_sequence`, redacted for `actor`.
    pub(crate) fn events_after(
        &self,
        actor: &UserId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        let observer = self.seat_for(actor)?;
        let events = self
            .events
            .iter()
            .filter(|record| record.sequence > after_sequence)
            .take(MATCH_EVENT_PAGE_LIMIT)
            .map(|record| MatchEvent::redacted(record, observer))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(MatchEventPage::new(
            self.version,
            self.event_sequence,
            events,
        ))
    }

    /// 等不到某一家load完就把整局作废，返回这一次是否真的作废了。
    ///
    /// 和开局动画那段宽限不同：这里等的是网络下载，慢的网要几十秒，所以给得长
    /// 得多。真等不到的那家八成已经断了，剩下三家干等下去没有意义，整局作废，
    /// 各自回房间。
    pub(crate) fn terminate_if_assets_stalled(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.is_finished() || !self.opening.terminate_if_assets_stalled(now_ms) {
            return Ok(false);
        }
        for clock in &mut self.clocks {
            clock.disarm(now_ms);
        }
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        Ok(true)
    }

    #[must_use]
    pub(crate) fn is_finished(&self) -> bool {
        self.game.result().is_some()
            || self.terminated_by_exit_vote
            || self.opening.terminated_by_asset_timeout()
    }

    #[must_use]
    pub(crate) const fn has_pending_settlement(&self) -> bool {
        self.pending_settlement.is_some()
    }

    pub(crate) fn seat_for(&self, user_id: &UserId) -> Result<Seat, ApplicationError> {
        self.players
            .iter()
            .find(|player| &player.user_id == user_id)
            .map(|player| player.seat)
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::NotMatchPlayer,
                    "user is not a player in this match",
                )
            })
    }
}

fn invalid_command(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidGameCommand, error.to_string())
}

fn internal_error(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, message)
}

pub(crate) fn shuffle_players<T>(players: &mut [T]) -> Result<(), ApplicationError> {
    for upper in (1..players.len()).rev() {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes)
            .map_err(|error| internal_error(format!("seat randomization failed: {error}")))?;
        let random = u64::from_le_bytes(bytes);
        let index =
            usize::try_from(random % u64::try_from(upper + 1).expect("seat count fits u64"))
                .expect("bounded random seat index fits usize");
        players.swap(upper, index);
    }
    Ok(())
}
