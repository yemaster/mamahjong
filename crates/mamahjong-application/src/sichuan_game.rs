//! 四川麻将（血战到底）的对局运行时。
//!
//! 这里只实现四川麻将自己的牌局状态、合法操作、计分和投影。素材握手、开局动画
//! 放行与结算确认由 `match_flow` 提供，换三张与定缺两阶段也在 `match_flow` 里
//! 封装成可复用的流程；整场调度通过 `RuleRuntime` 接入应用层。
//!
//! 投影一律用 `u8` 座位号与牌码字符串，DTO 层直接照抄即可，不必再认识引擎类型。

use mahjong_core::{MatchId, RoomId, UserId};
use mahjong_sichuan::{
    HandPhase, MeldId, ReactionKind, Seat, SichuanHand, SichuanMatch, SichuanRuleSnapshot,
    SichuanRules, Suit, TileId, TurnAction, WallSeed,
};

/// 换三张动画等四家播完再放行到定缺；完整飞出/换位/飞入演出约 5.5 s，兜底留足余量。
const EXCHANGE_ANIMATION_FALLBACK_MS: u64 = 10_000;
const KAN_ANIMATION_FALLBACK_MS: u64 = 6_000;
const WIN_ANIMATION_FALLBACK_MS: u64 = 8_000;

use crate::clock::SeatClock;
use crate::game::{GameCommand, SubmitGameCommand, shuffle_players};
use crate::match_flow::{DingQueFlow, ExchangeFlow, MatchOpening, SettlementFlow};
use crate::presentation::{
    animation_grace_ms, settlement_fallback_ms, settlement_reveal_fallback_ms,
};
use crate::{ApplicationError, ErrorCode, Room, RoomLifecycle};

const EXIT_VOTE_DURATION_MS: u64 = 15_000;
const SEATS: usize = mahjong_sichuan::SEAT_COUNT as usize;

/// 一位在座玩家的身份，和立直那边的 `MatchPlayer` 同形，只是座位是 `u8`。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SichuanPlayer {
    user_id: UserId,
    seat: u8,
    nickname: String,
}

impl SichuanPlayer {
    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn seat(&self) -> u8 {
        self.seat
    }

    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }
}

/// 一张牌的投影：牌码与立直那边完全一致，前端的贴图表可以直接复用。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SichuanTileView {
    pub id: u16,
    pub code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SichuanMeldView {
    pub id: u16,
    pub kind: &'static str,
    pub tiles: Vec<SichuanTileView>,
    pub called_from: Option<u8>,
    pub called_tile_id: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SichuanDiscardView {
    pub tile: SichuanTileView,
    pub called: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanPlayer {
    pub player: SichuanPlayer,
    pub points: i32,
    /// 定缺门，`man` / `pin` / `sou` 之一；四家定缺完成前统一隐藏。
    pub que_suit: Option<&'static str>,
    /// 已经胡牌。胡后盖牌退出，其余继续。
    pub won: bool,
    /// 胡的那张牌，前端标成浅红色。未胡为空。
    pub winning_tile: Option<SichuanTileView>,
    /// 这家是否自摸；胡后 `drawn_tile_id` 会隐藏，前端不能再靠牌号反推。
    pub win_is_tsumo: Option<bool>,
    pub concealed_tile_count: usize,
    /// 只有本人（或结算摊牌时）看得到具体是哪几张；胡牌家盖牌时对外始终为空。
    pub concealed_tiles: Option<Vec<SichuanTileView>>,
    pub drawn_tile_id: Option<u16>,
    pub melds: Vec<SichuanMeldView>,
    pub discards: Vec<SichuanDiscardView>,
    /// 本局已经成立的杠数（根）。
    pub kan_count: u8,
}

/// 轮到自己时的合法选项。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SichuanTurnActionsView {
    pub can_tsumo: bool,
    /// 可以暗杠的牌码。
    pub concealed_kans: Vec<String>,
    /// 可以加杠的副露编号。
    pub added_kans: Vec<u16>,
    /// 打哪张能听：(打出的牌 id, 打出后能和的牌码列表)。
    pub tenpai_discard_hints: Vec<(u16, Vec<String>)>,
}

/// 对别家打出的那张牌可以做的事。四川麻将没有吃。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SichuanReactionOptionsView {
    pub can_ron: bool,
    pub can_pon: bool,
    pub can_open_kan: bool,
}

/// 一次杠（雨）的点数变动，供前端播浮层。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObserverSichuanKanEvent {
    /// 递增序号：客户端靠它认出「这是新的一次」。
    pub id: u64,
    pub seat: u8,
    pub kind: &'static str,
    pub deltas: [i32; SEATS],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanWinEvent {
    /// 递增序号：客户端靠它区分新胡牌与同一视图的重发。
    pub id: u64,
    pub seat: u8,
    pub is_tsumo: bool,
    pub payer: Option<u8>,
    pub chankan: bool,
    pub winning_tile: Option<SichuanTileView>,
    pub deltas: [i32; SEATS],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanWinner {
    pub seat: u8,
    pub is_tsumo: bool,
    pub payer: Option<u8>,
    pub chankan: bool,
    pub winning_tile: Option<SichuanTileView>,
    pub yaku: Vec<ObserverSichuanYaku>,
    /// 番数（封顶 6 番）。
    pub fan: u32,
    /// 分数（2^(番-1)）。
    pub score: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanYaku {
    pub name: &'static str,
    pub value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanQue {
    pub flower_pigs: Vec<u8>,
    pub tenpai: Vec<u8>,
    pub noten: Vec<u8>,
    pub deltas: [i32; SEATS],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanSettlement {
    pub reason: &'static str,
    /// 本局所有胡家，按胡牌顺序。血战到底一家胡后继续。
    pub winners: Vec<ObserverSichuanWinner>,
    /// 流局时的查花猪 / 查大叫；非流局为空。
    pub que: Option<ObserverSichuanQue>,
    pub point_deltas: [i32; SEATS],
    pub points_after: [i32; SEATS],
    pub played_seats: Vec<u8>,
    pub confirm_deadline_ms: Option<u64>,
    pub confirmed_seats: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanResult {
    pub final_points: [i32; SEATS],
    pub point_deltas: [i32; SEATS],
    /// 按剩余点数从高到低排的座位号。
    pub placements: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanExitVote {
    pub initiator: u8,
    pub deadline_ms: u64,
    pub votes: Vec<Option<bool>>,
}

/// 一位观察者看到的四川麻将牌桌。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverSichuanMatch {
    pub id: MatchId,
    pub room_id: RoomId,
    pub observer_seat: u8,
    pub version: u64,
    pub event_sequence: u64,
    pub hand_index: u32,
    pub dealer: u8,
    /// 本局用的整套规则，前端按它决定按钮文案与建房面板回显。
    pub rules: SichuanRules,
    pub phase_kind: &'static str,
    pub phase_seat: Option<u8>,
    pub phase_reason: Option<&'static str>,
    /// 换三张方向，`counter_clockwise` / `clockwise` / `opposite`。
    pub exchange_direction: &'static str,
    /// 本局实际掷出的两颗骰子；前端开场展示与换牌方向必须使用同一份结果。
    pub exchange_dice: [u8; 2],
    pub break_seat: u8,
    pub remaining_draws: usize,
    pub completed_rinshan_draws: usize,
    pub players: Vec<ObserverSichuanPlayer>,
    pub reaction_options: SichuanReactionOptionsView,
    pub turn_actions: SichuanTurnActionsView,
    pub clocks: Vec<SeatClock>,
    pub opening_ready_seats: Vec<u8>,
    pub assets_ready_seats: Vec<u8>,
    pub terminated_by_asset_timeout: bool,
    /// 换三张阶段已提交 3 张的座位。
    pub exchange_submitted_seats: Vec<u8>,
    /// 观察者自己交出的三张牌；用于超时或断线后的换牌动画重建。
    pub exchange_outgoing_tile_ids: Option<Vec<u16>>,
    /// 已播完换三张动画的座位。
    pub exchange_animation_played_seats: Vec<u8>,
    /// 定缺阶段已提交定缺门的座位。
    pub dingque_submitted_seats: Vec<u8>,
    /// 最近一次胡牌的即时动画事件；会持续到下一次胡牌/换局，客户端按 id 去重。
    pub last_win: Option<ObserverSichuanWinEvent>,
    /// 换牌/定缺阶段的统一截止时刻；普通打牌仍使用各家独立时钟。
    pub phase_deadline_ms: Option<u64>,
    pub hand_settlement: Option<ObserverSichuanSettlement>,
    pub last_kan: Option<ObserverSichuanKanEvent>,
    pub result: Option<ObserverSichuanResult>,
    pub friend_match: bool,
    pub can_start_exit_vote: bool,
    pub exit_vote: Option<ObserverSichuanExitVote>,
    pub terminated_by_exit_vote: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExitVote {
    initiator: u8,
    deadline_ms: u64,
    votes: Box<[Option<bool>]>,
    paused_clock_elapsed_ms: Box<[Option<i64>]>,
}

/// 杠点动画播放状态：记录是哪次杠、谁要摸岭上牌、哪些座位已报告播完。
#[derive(Clone, Debug)]
struct PendingKanAnimation {
    kan_id: u64,
    seat: Seat,
    started_at_ms: u64,
    played: [bool; SEATS],
}

#[derive(Clone, Debug)]
struct PendingWinAnimation {
    win_id: u64,
    seat: Seat,
    started_at_ms: u64,
    played: [bool; SEATS],
}

#[derive(Clone, Debug)]
struct PendingSettlement {
    settlement: mahjong_sichuan::HandSettlement,
    /// 本局的庄。`settle_hand()` 当场把庄挪走，结算画面还铺在桌上时先存这儿，
    /// 等四家点完确认、开下一局再露出去。
    dealer: u8,
    flow: SettlementFlow,
}

#[derive(Clone, Debug)]
pub(crate) struct SichuanRuntime {
    pub(crate) id: MatchId,
    pub(crate) room_id: RoomId,
    pub(crate) version: u64,
    /// 四川麻将暂不生成事件流，游标恒为 0；客户端走视图订阅。
    pub(crate) event_sequence: u64,
    hand_index: u32,
    pub(crate) players: Box<[SichuanPlayer]>,
    pub(crate) rule_snapshot: SichuanRuleSnapshot,
    game: SichuanMatch,
    result: Option<ObserverSichuanResult>,
    clocks: Box<[SeatClock]>,
    opening: MatchOpening,
    exchange: ExchangeFlow,
    dingque: DingQueFlow,
    pending: Option<PendingSettlement>,
    pending_kan_animation: Option<PendingKanAnimation>,
    pending_win_animation: Option<PendingWinAnimation>,
    last_kan: Option<ObserverSichuanKanEvent>,
    kan_sequence: u64,
    win_sequence: u64,
    phase_deadline_ms: Option<u64>,
    phase_timeout_ms: u64,
    last_win: Option<ObserverSichuanWinEvent>,
    /// 各家胡的那张牌（浅红色），按座位索引；结算摊牌前用来盖牌并高亮胡张。
    winning_tiles: [Option<SichuanTileView>; SEATS],
    /// 加杠进入抢杠窗口后，被抢的那张牌；窗口关闭即清空。
    pending_chankan_tile: Option<SichuanTileView>,
    pub(crate) friend_match: bool,
    exit_vote_used_hand: [Option<u32>; SEATS],
    exit_vote: Option<ExitVote>,
    terminated_by_exit_vote: bool,
}

impl SichuanRuntime {
    pub(crate) fn start(room: &Room, id: MatchId, now_ms: u64) -> Result<Self, ApplicationError> {
        if room.lifecycle() != RoomLifecycle::Playing || room.active_match_id() != Some(&id) {
            return Err(internal_error("room is not linked to the starting match"));
        }
        let snapshot = room
            .rule_snapshot()
            .as_sichuan()
            .ok_or_else(|| internal_error("room does not carry a sichuan rule snapshot"))?;
        let rules = *snapshot.rules();
        let thinking_time = rules.match_rules.thinking_time;
        let mut game = SichuanMatch::new(rules);
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        game.start_hand(&seed)
            .map_err(|error| internal_error(error.to_string()))?;

        let mut members = room.members().iter().collect::<Vec<_>>();
        shuffle_players(&mut members)?;
        if members.len() != SEATS {
            return Err(internal_error("sichuan matches need exactly four players"));
        }
        let players = members
            .into_iter()
            .enumerate()
            .map(|(seat_index, member)| {
                Ok(SichuanPlayer {
                    user_id: member.user_id().clone(),
                    seat: u8::try_from(seat_index)
                        .map_err(|_| internal_error("seat index exceeds u8"))?,
                    nickname: member.nickname().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?
            .into_boxed_slice();

        let mut runtime = Self {
            id,
            room_id: room.id().clone(),
            version: 1,
            event_sequence: 0,
            hand_index: 0,
            players,
            rule_snapshot: snapshot.clone(),
            game,
            result: None,
            clocks: vec![
                SeatClock::with_limits(thinking_time.base_ms(), thinking_time.reserve_ms());
                SEATS
            ]
            .into_boxed_slice(),
            opening: MatchOpening::new(SEATS, now_ms),
            exchange: ExchangeFlow::new(SEATS),
            dingque: DingQueFlow::new(SEATS),
            pending: None,
            pending_kan_animation: None,
            pending_win_animation: None,
            last_kan: None,
            kan_sequence: 0,
            win_sequence: 0,
            phase_deadline_ms: None,
            phase_timeout_ms: thinking_time
                .base_ms()
                .saturating_add(u64::from(thinking_time.reserve_ms())),
            last_win: None,
            winning_tiles: std::array::from_fn(|_| None),
            pending_chankan_tile: None,
            friend_match: !room.is_matchmaking_room(),
            exit_vote_used_hand: [None; SEATS],
            exit_vote: None,
            terminated_by_exit_vote: false,
        };
        runtime.rearm_clocks(now_ms)?;
        Ok(runtime)
    }

    fn seat(&self, user_id: &UserId) -> Result<u8, ApplicationError> {
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

    pub(crate) fn seat_for(&self, user_id: &UserId) -> Result<u8, ApplicationError> {
        self.seat(user_id)
    }

    /// 开发/测试专用：把该玩家暗手整体换成给定牌码。走权威状态，改的是牌面、牌 id 不变。
    pub(crate) fn set_dev_hand(
        &mut self,
        actor: &UserId,
        codes: &[String],
    ) -> Result<(), ApplicationError> {
        let seat = self.seat(actor)?;
        let hand = self
            .game
            .hand_mut()
            .ok_or_else(|| internal_error("there is no hand in progress"))?;
        hand.set_concealed_tiles(seat_of(seat)?, codes)
            .map_err(|error| internal_error(error.to_string()))?;
        self.version = self.version.saturating_add(1);
        Ok(())
    }

    fn assets_loading(&self) -> bool {
        self.opening.assets_loading()
    }

    fn frozen(&self) -> bool {
        self.exit_vote.is_some()
            || self.terminated_by_exit_vote
            || self.assets_loading()
            || self.opening.terminated_by_asset_timeout()
    }

    pub(crate) fn view(&self, actor: &UserId) -> Result<ObserverSichuanMatch, ApplicationError> {
        let observer_seat = self.seat(actor)?;
        let hand = self.game.hand();
        /*
         * 定缺是同时公开的选择。引擎内部仍然需要逐家记录选择，才能校验每一条
         * 指令；投影层则在四家都提交、阶段离开 AwaitingDingQue 之后才把花色放
         * 到客户端。这样既保留了自己的“已提交”进度，也不会让先提交的人泄露
         * 选择给其他玩家。
         */
        let dingque_revealed = !matches!(
            hand.map(SichuanHand::phase),
            Some(HandPhase::AwaitingDingQue)
        );
        let revealed = self.pending.is_some() || self.result.is_some();
        // 结算动画期间，点数维持上一局的值，动画才是从旧值滚到新值。
        let base_points: [i32; SEATS] = if let Some(ref pending) = self.pending {
            let settlement = &pending.settlement;
            let mut points = [0_i32; SEATS];
            for (index, point) in points.iter_mut().enumerate() {
                *point = settlement.points_after()[index] - settlement.point_deltas()[index];
            }
            points
        } else {
            // 杠/胡会先记入当前手的即时账，整局结束时才并入比赛总分。
            // 投影必须把这笔账带上，否则点数动画的终点会落回本局初始分。
            let mut points = *self.game.points();
            if let Some(hand) = self.game.hand() {
                for (index, delta) in hand.point_deltas().iter().copied().enumerate() {
                    points[index] += delta;
                }
            }
            points
        };
        let players = self
            .players
            .iter()
            .map(|player| {
                let index = usize::from(player.seat);
                let seat = seat_of(player.seat)?;
                let (
                    concealed_tile_count,
                    concealed_tiles,
                    drawn_tile_id,
                    melds,
                    discards,
                    kan_count,
                ) = hand.map_or_else(
                    || (0, None, None, Vec::new(), Vec::new(), 0),
                    |hand| {
                        let held = hand.player(seat);
                        let mine = player.seat == observer_seat;
                        let won = hand.won(seat);
                        // 胡牌家盖牌：本局继续打，别人（含自己）都不该看到底牌，等结算摊牌。
                        let show = (mine && !won) || revealed;
                        (
                            held.concealed().len(),
                            show.then(|| held.concealed().iter().copied().map(tile_view).collect()),
                            show.then(|| held.drawn().map(TileId::value)).flatten(),
                            held.melds().iter().map(meld_view).collect(),
                            held.discards()
                                .iter()
                                .map(|discard| SichuanDiscardView {
                                    tile: tile_view(discard.tile()),
                                    called: discard.called(),
                                })
                                .collect(),
                            held.kan_count(),
                        )
                    },
                );
                Ok(ObserverSichuanPlayer {
                    player: player.clone(),
                    points: base_points[index],
                    que_suit: dingque_revealed
                        .then(|| hand.and_then(|hand| hand.que_suit(seat)))
                        .flatten()
                        .map(Suit::as_str),
                    won: hand.is_some_and(|hand| hand.won(seat)),
                    winning_tile: self.winning_tiles[index].clone(),
                    win_is_tsumo: hand.and_then(|hand| {
                        hand.winners()
                            .iter()
                            .find(|winner| winner.seat() == seat)
                            .map(|winner| winner.is_tsumo())
                    }),
                    concealed_tile_count,
                    concealed_tiles,
                    drawn_tile_id,
                    melds,
                    discards,
                    kan_count,
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?;

        let observer = seat_of(observer_seat)?;
        let (phase_kind, phase_seat, phase_reason) =
            hand.map_or(("awaiting_exchange", None, None), |hand| {
                match hand.phase() {
                    HandPhase::AwaitingExchange => ("awaiting_exchange", None, None),
                    HandPhase::AwaitingExchangeAnimation => {
                        ("awaiting_exchange_animation", None, None)
                    }
                    HandPhase::AwaitingDingQue => ("awaiting_dingque", None, None),
                    HandPhase::AwaitingTurnAction { seat } => {
                        ("awaiting_turn_action", Some(seat.index()), None)
                    }
                    HandPhase::AwaitingDiscard { seat } => {
                        ("awaiting_discard", Some(seat.index()), None)
                    }
                    HandPhase::AwaitingResponses { discarder } => {
                        ("awaiting_responses", Some(discarder.index()), None)
                    }
                    HandPhase::AwaitingKanAnimation { seat } => {
                        ("awaiting_kan_animation", Some(seat.index()), None)
                    }
                    HandPhase::AwaitingWinAnimation { seat } => {
                        ("awaiting_win_animation", Some(seat.index()), None)
                    }
                    HandPhase::Ended { reason } => ("ended", None, Some(reason.as_str())),
                }
            });

        let (turn_actions, reaction_options) = if self.frozen()
            || self.pending.is_some()
            || self.pending_kan_animation.is_some()
            || self.pending_win_animation.is_some()
        {
            (
                SichuanTurnActionsView::default(),
                SichuanReactionOptionsView::default(),
            )
        } else {
            hand.map_or_else(
                || {
                    (
                        SichuanTurnActionsView::default(),
                        SichuanReactionOptionsView::default(),
                    )
                },
                |hand| {
                    let actions = hand.turn_actions(observer);
                    let options = hand.reaction_options(observer);
                    (
                        SichuanTurnActionsView {
                            can_tsumo: actions.can_tsumo,
                            concealed_kans: actions
                                .concealed_kans
                                .iter()
                                .map(ToString::to_string)
                                .collect(),
                            added_kans: actions
                                .added_kans
                                .iter()
                                .map(|meld| meld.value())
                                .collect(),
                            tenpai_discard_hints: actions
                                .tenpai_discard_hints
                                .iter()
                                .map(|(tile_id, kinds)| {
                                    (
                                        tile_id.value(),
                                        kinds.iter().map(ToString::to_string).collect(),
                                    )
                                })
                                .collect(),
                        },
                        SichuanReactionOptionsView {
                            can_ron: options.can_ron,
                            can_pon: options.can_pon,
                            can_open_kan: options.can_open_kan,
                        },
                    )
                },
            )
        };

        Ok(ObserverSichuanMatch {
            id: self.id.clone(),
            room_id: self.room_id.clone(),
            observer_seat,
            version: self.version,
            event_sequence: self.event_sequence,
            hand_index: self.hand_index,
            dealer: self
                .pending
                .as_ref()
                .map_or_else(|| self.game.progress().dealer().index(), |p| p.dealer),
            rules: *self.rule_snapshot.rules(),
            phase_kind,
            phase_seat,
            phase_reason,
            exchange_direction: hand.map_or("counter_clockwise", |hand| {
                hand.exchange_direction().as_str()
            }),
            exchange_dice: hand.map_or([1, 1], |hand| {
                let dice = hand.dice();
                [dice.first(), dice.second()]
            }),
            break_seat: hand.map_or(0, |hand| hand.break_seat().index()),
            remaining_draws: hand.map_or(0, SichuanHand::remaining_draws),
            completed_rinshan_draws: hand.map_or(0, SichuanHand::completed_rinshan_draws),
            players,
            reaction_options,
            turn_actions,
            clocks: self.clocks.to_vec(),
            opening_ready_seats: seats_with_flag(self.opening.opening_ready_flags()),
            assets_ready_seats: seats_with_flag(self.opening.assets_ready_flags()),
            terminated_by_asset_timeout: self.opening.terminated_by_asset_timeout(),
            exchange_submitted_seats: seats_with_flag(self.exchange.submitted_flags()),
            exchange_outgoing_tile_ids: self
                .exchange
                .submitted_tile_ids(usize::from(observer_seat))
                .map(|ids| ids.to_vec()),
            exchange_animation_played_seats: seats_with_flag(
                self.exchange.animation_played_flags(),
            ),
            dingque_submitted_seats: seats_with_flag(self.dingque.submitted_flags()),
            last_win: self.last_win.clone(),
            phase_deadline_ms: self.phase_deadline_ms,
            hand_settlement: self
                .pending
                .as_ref()
                .map(|pending| self.settlement_view(pending)),
            last_kan: self.last_kan,
            result: self.result.clone(),
            friend_match: self.friend_match,
            can_start_exit_vote: self.friend_match
                && !self.assets_loading()
                && !self.opening.terminated_by_asset_timeout()
                && self.exit_vote.is_none()
                && self.exit_vote_used_hand[usize::from(observer_seat)] != Some(self.hand_index)
                && !self.terminated_by_exit_vote
                && self.pending_kan_animation.is_none()
                && self.pending.is_none()
                && self.result.is_none(),
            exit_vote: self.exit_vote.as_ref().map(|vote| ObserverSichuanExitVote {
                initiator: vote.initiator,
                deadline_ms: vote.deadline_ms,
                votes: vote.votes.to_vec(),
            }),
            terminated_by_exit_vote: self.terminated_by_exit_vote,
        })
    }

    fn settlement_view(&self, pending: &PendingSettlement) -> ObserverSichuanSettlement {
        let settlement = &pending.settlement;
        ObserverSichuanSettlement {
            reason: settlement.reason().as_str(),
            winners: settlement
                .winners()
                .iter()
                .map(|winner| {
                    let evaluation = winner.evaluation();
                    ObserverSichuanWinner {
                        seat: winner.seat().index(),
                        is_tsumo: winner.is_tsumo(),
                        payer: winner.payer().map(Seat::index),
                        chankan: winner.is_chankan(),
                        winning_tile: self.winning_tiles[usize::from(winner.seat().index())]
                            .clone(),
                        yaku: evaluation
                            .yaku()
                            .iter()
                            .map(|value| ObserverSichuanYaku {
                                name: crate::naming::sichuan_yaku_name(value.yaku()),
                                value: value.fan(),
                            })
                            .collect(),
                        fan: evaluation.fan(),
                        score: evaluation.score(),
                    }
                })
                .collect(),
            que: settlement.que().map(|que| ObserverSichuanQue {
                flower_pigs: que.flower_pigs().iter().map(|seat| seat.index()).collect(),
                tenpai: que.tenpai().iter().map(|seat| seat.index()).collect(),
                noten: que.noten().iter().map(|seat| seat.index()).collect(),
                deltas: *que.deltas(),
            }),
            point_deltas: *settlement.point_deltas(),
            points_after: *settlement.points_after(),
            played_seats: seats_with_flag(pending.flow.played_flags()),
            confirm_deadline_ms: pending.flow.confirm_deadline_ms(),
            confirmed_seats: seats_with_flag(pending.flow.confirmed_flags()),
        }
    }

    pub(crate) fn execute(
        &mut self,
        actor: &UserId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        let seat = self.seat(actor)?;
        let index = usize::from(seat);
        let handshake = matches!(
            command.command,
            GameCommand::MatchAssetsReady
                | GameCommand::ReadyForHand { .. }
                | GameCommand::SettlementPlayed { .. }
                | GameCommand::ConfirmSettlement { .. }
                | GameCommand::SichuanExchangeAnimationPlayed
                | GameCommand::SichuanWinAnimationPlayed { .. }
                | GameCommand::SichuanKanAnimationPlayed { .. }
        );
        if !handshake && command.expected_version != self.version {
            return Err(ApplicationError::new(
                ErrorCode::MatchVersionConflict,
                format!(
                    "expected match version {}, current version is {}",
                    command.expected_version, self.version
                ),
            ));
        }
        if self.opening.terminated_by_asset_timeout() {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match was terminated while waiting for players to load",
            ));
        }
        if matches!(command.command, GameCommand::MatchAssetsReady) {
            let report = self.opening.report_assets_ready(index, now_ms);
            if !report.changed() {
                return Ok(());
            }
            if report.everyone_ready() {
                self.rearm_clocks(now_ms)?;
            }
            return self.bump_version();
        }
        if self.assets_loading() {
            return Err(invalid_command("players are still loading match assets"));
        }
        match command.command {
            GameCommand::RequestExitVote => {
                self.request_exit_vote(seat, now_ms)?;
                return self.bump_version();
            }
            GameCommand::VoteExit { agree } => {
                self.cast_exit_vote(seat, agree, now_ms)?;
                return self.bump_version();
            }
            _ if self.exit_vote.is_some() => {
                return Err(invalid_command("the match is paused for an exit vote"));
            }
            _ => {}
        }
        match command.command {
            GameCommand::ReadyForHand { hand_index } => {
                if hand_index != self.hand_index {
                    return Err(invalid_command(
                        "the opening animation is no longer current",
                    ));
                }
                let report = self.opening.report_opening_ready(index, now_ms);
                if !report.changed() {
                    return Ok(());
                }
                if report.everyone_ready() {
                    self.begin_exchange_phase(now_ms);
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            GameCommand::SettlementPlayed { hand_index } => {
                if hand_index != self.hand_index {
                    return Err(invalid_command("the hand settlement is no longer current"));
                }
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or_else(|| invalid_command("there is no hand settlement being played"))?;
                let report = pending.flow.report_played(index, now_ms);
                if !report.changed() {
                    return Ok(());
                }
                return self.bump_version();
            }
            GameCommand::ConfirmSettlement { hand_index } => {
                if hand_index != self.hand_index {
                    return Err(invalid_command("the hand settlement is no longer current"));
                }
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or_else(|| invalid_command("there is no hand settlement to confirm"))?;
                if !pending.flow.confirmation_open() {
                    return Err(invalid_command("the settlement is still being played"));
                }
                let report = pending.flow.report_confirmed(index);
                if !report.changed() {
                    return Ok(());
                }
                if report.everyone_ready() {
                    self.advance_settlement(now_ms)?;
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            GameCommand::SichuanExchangeAnimationPlayed => {
                let phase = self.game.hand().map(SichuanHand::phase);
                if !matches!(
                    phase,
                    Some(HandPhase::AwaitingExchangeAnimation | HandPhase::AwaitingDingQue)
                ) {
                    return Err(invalid_command("the exchange animation is not pending"));
                }
                let report = self.exchange.report_animation_played(index);
                if !report.changed() {
                    return Ok(());
                }
                /* 四家前端都报告后，才把规则阶段推进到定缺；前端收到新阶段后再
                显示定缺面板。旧状态若已经在定缺，重复回执保持幂等。 */
                if report.everyone_ready()
                    && matches!(phase, Some(HandPhase::AwaitingExchangeAnimation))
                {
                    self.game
                        .hand_mut()
                        .ok_or_else(|| internal_error("there is no hand in progress"))?
                        .advance_from_exchange_animation()
                        .map_err(|error| internal_error(error.to_string()))?;
                    self.begin_dingque_phase(now_ms);
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            GameCommand::SichuanWinAnimationPlayed { win_id } => {
                let pending = self
                    .pending_win_animation
                    .as_mut()
                    .ok_or_else(|| invalid_command("there is no win animation pending"))?;
                if pending.win_id != win_id {
                    return Err(invalid_command(
                        "the win animation id does not match the current win",
                    ));
                }
                if pending.played[index] {
                    return Ok(());
                }
                pending.played[index] = true;
                if pending.played.iter().all(|played| *played) {
                    self.advance_win_animation(now_ms)?;
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            GameCommand::SichuanKanAnimationPlayed { kan_id } => {
                let pending = self
                    .pending_kan_animation
                    .as_mut()
                    .ok_or_else(|| invalid_command("there is no kan animation pending"))?;
                if pending.kan_id != kan_id {
                    return Err(invalid_command(
                        "the kan animation id does not match the current kan",
                    ));
                }
                if pending.played[index] {
                    return Ok(());
                }
                pending.played[index] = true;
                if pending.played.iter().all(|played| *played) {
                    self.advance_kan_animation()?;
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            _ => {}
        }
        if self.is_finished() {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match is already finished",
            ));
        }
        if self.pending.is_some() {
            return Err(invalid_command("the hand settlement is still on screen"));
        }
        if self.pending_kan_animation.is_some() {
            return Err(invalid_command(
                "waiting for all players to finish the kan animation",
            ));
        }
        if self.pending_win_animation.is_some() {
            return Err(invalid_command(
                "waiting for all players to finish the win animation",
            ));
        }
        if self.opening.opening_blocked() {
            return Err(invalid_command("the opening deal is still being dealt"));
        }

        // 换三张与定缺：各走自己的流程封装，走完直接收尾，不经过打牌指令。
        if let GameCommand::SichuanExchange { tile_ids } = &command.command {
            self.apply_exchange(seat, *tile_ids, now_ms)?;
            return self.finish_command(index, now_ms, 0);
        }
        if let GameCommand::SichuanDingQue { suit } = &command.command {
            self.apply_dingque(seat, suit)?;
            return self.finish_command(index, now_ms, 0);
        }

        let grace_ms = animation_grace_ms(&command.command);
        let actor_seat = seat_of(seat)?;
        let kan_events_before = self
            .game
            .hand()
            .ok_or_else(|| invalid_command("there is no hand in progress"))?
            .kan_events()
            .len();
        let winner = self.apply_hand_command(actor_seat, &command.command)?;

        let (is_ended, kan_animation_seat, win_animation_seat, new_kan_event, win_record) = {
            let hand = self
                .game
                .hand()
                .ok_or_else(|| internal_error("the hand vanished mid-command"))?;
            let is_ended = hand.phase().is_ended();
            let kan_animation_seat = match hand.phase() {
                HandPhase::AwaitingKanAnimation { seat } => Some(seat),
                _ => None,
            };
            let win_animation_seat = match hand.phase() {
                HandPhase::AwaitingWinAnimation { seat } => Some(seat),
                _ => None,
            };
            let new_kan_event = if hand.kan_events().len() > kan_events_before {
                let event = hand
                    .kan_events()
                    .last()
                    .expect("the kan event list just grew");
                Some((event.seat().index(), event.kind().as_str(), *event.deltas()))
            } else {
                None
            };
            let win_record = winner
                .as_ref()
                .and_then(|_| hand.winners().last())
                .map(|record| {
                    (
                        record.is_tsumo(),
                        record.payer(),
                        record.is_chankan(),
                        *record.deltas(),
                    )
                });
            (
                is_ended,
                kan_animation_seat,
                win_animation_seat,
                new_kan_event,
                win_record,
            )
        };

        if let Some((seat, kind, deltas)) = new_kan_event {
            self.kan_sequence = self
                .kan_sequence
                .checked_add(1)
                .ok_or_else(|| internal_error("kan sequence overflow"))?;
            self.last_kan = Some(ObserverSichuanKanEvent {
                id: self.kan_sequence,
                seat,
                kind,
                deltas,
            });
        }
        if let Some((winner_seat, tile)) = winner {
            self.winning_tiles[usize::from(winner_seat.index())] = Some(tile.clone());
            if let Some((is_tsumo, payer, chankan, deltas)) = win_record {
                self.win_sequence = self
                    .win_sequence
                    .checked_add(1)
                    .ok_or_else(|| internal_error("win sequence overflow"))?;
                self.last_win = Some(ObserverSichuanWinEvent {
                    id: self.win_sequence,
                    seat: winner_seat.index(),
                    is_tsumo,
                    payer: payer.map(Seat::index),
                    chankan,
                    winning_tile: Some(tile),
                    deltas,
                });
            }
        }
        if is_ended {
            self.finish_hand(now_ms)?;
        } else if let Some(pending_seat) = kan_animation_seat {
            // 杠完还没结束局：把当前的 kan_sequence 记进挂起状态，等四家报告动画播完。
            self.pending_kan_animation = Some(PendingKanAnimation {
                kan_id: self.kan_sequence,
                seat: pending_seat,
                started_at_ms: now_ms,
                played: [false; SEATS],
            });
        } else if let Some(pending_seat) = win_animation_seat {
            self.pending_win_animation = Some(PendingWinAnimation {
                win_id: self.win_sequence,
                seat: pending_seat,
                started_at_ms: now_ms,
                played: [false; SEATS],
            });
        }
        self.finish_command(index, now_ms, grace_ms)
    }

    fn finish_command(
        &mut self,
        actor_index: usize,
        now_ms: u64,
        grace_ms: u64,
    ) -> Result<(), ApplicationError> {
        self.bump_version()?;
        self.clocks[actor_index].disarm(now_ms);
        self.rearm_clocks_after(now_ms, grace_ms)
    }

    fn apply_exchange(
        &mut self,
        seat: u8,
        tile_ids: [u16; 3],
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        let index = usize::from(seat);
        {
            let hand = self
                .game
                .hand_mut()
                .ok_or_else(|| invalid_command("there is no hand in progress"))?;
            hand.submit_exchange(seat_of(seat)?, tile_ids.map(TileId::new))
                .map_err(invalid_command)?;
        }
        let report = self.exchange.report_submitted(index, tile_ids, now_ms);
        if report.everyone_ready()
            && matches!(
                self.game.hand().map(SichuanHand::phase),
                Some(HandPhase::AwaitingExchangeAnimation)
            )
        {
            /* 四家交牌后只开启“等动画回执”的阶段。定缺必须等四家都播完，或动画
            兜底超时后再广播，不能在最后一家交牌时抢跑。 */
            self.begin_exchange_animation_phase(now_ms);
        }
        Ok(())
    }

    fn apply_dingque(&mut self, seat: u8, suit: &str) -> Result<(), ApplicationError> {
        let suit = parse_suit(suit).ok_or_else(|| invalid_command("unknown suit"))?;
        let index = usize::from(seat);
        {
            let hand = self
                .game
                .hand_mut()
                .ok_or_else(|| invalid_command("there is no hand in progress"))?;
            hand.submit_dingque(seat_of(seat)?, suit)
                .map_err(invalid_command)?;
        }
        self.dingque.report_submitted(index);
        if matches!(
            self.game.hand().map(SichuanHand::phase),
            Some(HandPhase::AwaitingTurnAction { .. })
        ) {
            self.phase_deadline_ms = None;
        }
        Ok(())
    }

    fn begin_exchange_phase(&mut self, now_ms: u64) {
        self.phase_deadline_ms = Some(now_ms.saturating_add(self.phase_timeout_ms));
    }

    fn begin_exchange_animation_phase(&mut self, now_ms: u64) {
        self.phase_deadline_ms = Some(now_ms.saturating_add(EXCHANGE_ANIMATION_FALLBACK_MS));
    }

    fn begin_dingque_phase(&mut self, now_ms: u64) {
        self.phase_deadline_ms = Some(now_ms.saturating_add(self.phase_timeout_ms));
    }

    /// 把一条打牌指令交给引擎；返回值是「这一步谁胡了、胡哪张」，用来盖牌与标红。
    fn apply_hand_command(
        &mut self,
        seat: Seat,
        command: &GameCommand,
    ) -> Result<Option<(Seat, SichuanTileView)>, ApplicationError> {
        match command {
            GameCommand::SichuanDiscard { tile_id } => {
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_turn_action(
                        seat,
                        TurnAction::Discard {
                            tile: TileId::new(*tile_id),
                        },
                    )
                    .map_err(invalid_command)?;
                Ok(None)
            }
            GameCommand::SichuanTsumo => {
                let winners_before = self
                    .game
                    .hand()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .winners()
                    .len();
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_turn_action(seat, TurnAction::Tsumo)
                    .map_err(invalid_command)?;
                let hand = self
                    .game
                    .hand()
                    .ok_or_else(|| internal_error("the hand vanished mid-command"))?;
                if hand.winners().len() <= winners_before {
                    return Ok(None);
                }
                let tile = drawn_tile_view(hand, seat)
                    .ok_or_else(|| internal_error("the tsumo winning tile is missing"))?;
                Ok(Some((seat, tile)))
            }
            GameCommand::SichuanRon => {
                let winners_before = self
                    .game
                    .hand()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .winners()
                    .len();
                let (chankan, winning_tile) = {
                    let hand = self
                        .game
                        .hand()
                        .ok_or_else(|| invalid_command("there is no hand in progress"))?;
                    let chankan = hand.last_discard().is_none();
                    let winning_tile = hand
                        .last_discard()
                        .map(|(_, tile)| tile_view(tile))
                        .or_else(|| self.pending_chankan_tile.clone());
                    (chankan, winning_tile)
                };
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_reaction(seat, ReactionKind::Ron)
                    .map_err(invalid_command)?;
                if chankan {
                    self.pending_chankan_tile = None;
                }
                let hand = self
                    .game
                    .hand()
                    .ok_or_else(|| internal_error("the hand vanished mid-command"))?;
                if hand.winners().len() <= winners_before {
                    return Ok(None);
                }
                let tile = winning_tile
                    .ok_or_else(|| internal_error("the ron winning tile is missing"))?;
                Ok(Some((seat, tile)))
            }
            GameCommand::SichuanPon => {
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_reaction(seat, ReactionKind::Pon)
                    .map_err(invalid_command)?;
                Ok(None)
            }
            GameCommand::SichuanOpenKan => {
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_reaction(seat, ReactionKind::OpenKan)
                    .map_err(invalid_command)?;
                Ok(None)
            }
            GameCommand::SichuanConcealedKan { tile_code } => {
                let tile = tile_code
                    .parse()
                    .map_err(|_| invalid_command("unknown tile code"))?;
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_turn_action(seat, TurnAction::ConcealedKan { tile })
                    .map_err(invalid_command)?;
                Ok(None)
            }
            GameCommand::SichuanAddedKan { meld_id } => {
                let meld_id = MeldId::new(*meld_id);
                // 加杠被抢的是暗手里第 4 张（与碰同点）：副露里找到点后取暗手里同点那张。
                let added_tile = {
                    let hand = self
                        .game
                        .hand()
                        .ok_or_else(|| invalid_command("there is no hand in progress"))?;
                    hand.player(seat)
                        .melds()
                        .iter()
                        .find(|meld| meld.id() == meld_id)
                        .and_then(|meld| {
                            hand.player(seat)
                                .concealed()
                                .iter()
                                .find(|tile| tile.kind() == meld.tile())
                                .copied()
                        })
                };
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_turn_action(seat, TurnAction::AddedKan { meld: meld_id })
                    .map_err(invalid_command)?;
                let chankan_open = matches!(
                    self.game.hand().map(SichuanHand::phase),
                    Some(HandPhase::AwaitingResponses { .. })
                );
                self.pending_chankan_tile = chankan_open.then(|| {
                    tile_view(
                        added_tile.expect("the added kan tile is present after a successful call"),
                    )
                });
                Ok(None)
            }
            GameCommand::SichuanPass => {
                let chankan = self
                    .game
                    .hand()
                    .is_some_and(|hand| hand.last_discard().is_none());
                self.game
                    .hand_mut()
                    .ok_or_else(|| invalid_command("there is no hand in progress"))?
                    .apply_reaction(seat, ReactionKind::Pass)
                    .map_err(invalid_command)?;
                if chankan
                    && matches!(
                        self.game.hand().map(SichuanHand::phase),
                        Some(HandPhase::AwaitingKanAnimation { .. })
                    )
                {
                    self.pending_chankan_tile = None;
                }
                Ok(None)
            }
            _ => Err(invalid_command(
                "this command is not part of sichuan mahjong",
            )),
        }
    }

    fn finish_hand(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        // 先把本局的庄记下来，`settle_hand()` 会当场把庄挪到下一家。
        let dealer = self.game.progress().dealer().index();
        let settlement = self
            .game
            .settle_hand()
            .map_err(|error| internal_error(error.to_string()))?;
        if settlement.match_over() {
            let results = self.game.results();
            let mut placements = results.iter().collect::<Vec<_>>();
            placements.sort_by(|left, right| {
                right
                    .points
                    .cmp(&left.points)
                    .then_with(|| left.seat.index().cmp(&right.seat.index()))
            });
            let mut final_points = [0_i32; SEATS];
            let mut point_deltas = [0_i32; SEATS];
            for result in &results {
                let index = usize::from(result.seat.index());
                final_points[index] = result.points;
                point_deltas[index] = result.point_delta;
            }
            self.result = Some(ObserverSichuanResult {
                final_points,
                point_deltas,
                placements: placements
                    .into_iter()
                    .map(|result| result.seat.index())
                    .collect(),
            });
        }
        self.pending = Some(PendingSettlement {
            settlement,
            dealer,
            flow: SettlementFlow::new(SEATS, now_ms),
        });
        Ok(())
    }

    fn advance_settlement(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        self.pending
            .take()
            .ok_or_else(|| invalid_command("there is no hand settlement to advance"))?;
        if self.result.is_some() {
            return Ok(());
        }
        self.hand_index = self
            .hand_index
            .checked_add(1)
            .ok_or_else(|| internal_error("hand index overflow"))?;
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        self.game
            .start_hand(&seed)
            .map_err(|error| internal_error(error.to_string()))?;
        let thinking_time = self.game.rules().match_rules.thinking_time;
        self.clocks.fill(SeatClock::with_limits(
            thinking_time.base_ms(),
            thinking_time.reserve_ms(),
        ));
        self.opening.reset_hand(now_ms);
        self.exchange = ExchangeFlow::new(SEATS);
        self.dingque = DingQueFlow::new(SEATS);
        self.last_kan = None;
        self.pending_kan_animation = None;
        self.pending_win_animation = None;
        self.phase_deadline_ms = None;
        self.last_win = None;
        self.winning_tiles = std::array::from_fn(|_| None);
        self.pending_chankan_tile = None;
        Ok(())
    }

    /// 摸岭上牌：把挂起的杠点动画状态消费掉，让 `seat` 从牌山尾部摸牌并进入新回合。
    fn advance_kan_animation(&mut self) -> Result<(), ApplicationError> {
        let pending = self
            .pending_kan_animation
            .take()
            .ok_or_else(|| internal_error("there is no kan animation to advance"))?;
        self.game
            .hand_mut()
            .ok_or_else(|| internal_error("there is no hand in progress"))?
            .advance_from_kan_animation(pending.seat)
            .map_err(|error| internal_error(error.to_string()))?;
        self.last_kan = None;
        Ok(())
    }

    fn advance_win_animation(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        let pending = self
            .pending_win_animation
            .take()
            .ok_or_else(|| internal_error("there is no win animation to advance"))?;
        self.game
            .hand_mut()
            .ok_or_else(|| internal_error("there is no hand in progress"))?
            .advance_from_win_animation(pending.seat)
            .map_err(|error| internal_error(error.to_string()))?;
        if self.game.hand().is_some_and(|hand| hand.phase().is_ended()) {
            self.finish_hand(now_ms)?;
        }
        Ok(())
    }

    fn advance_win_animation_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        let Some(pending) = self.pending_win_animation.as_ref() else {
            return Ok(false);
        };
        if now_ms
            < pending
                .started_at_ms
                .saturating_add(WIN_ANIMATION_FALLBACK_MS)
        {
            return Ok(false);
        }
        self.advance_win_animation(now_ms)?;
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    /// 兜底：杠点动画超时还没收齐四家回执，就强制摸岭上牌。
    fn advance_kan_animation_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        let Some(pending) = self.pending_kan_animation.as_ref() else {
            return Ok(false);
        };
        if now_ms
            < pending
                .started_at_ms
                .saturating_add(KAN_ANIMATION_FALLBACK_MS)
        {
            return Ok(false);
        }
        self.advance_kan_animation()?;
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    /// 兜底：换三张动画超时还没收齐四家回执，就强制放行到定缺。
    fn release_exchange_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        if self.pending.is_some() || self.result.is_some() {
            return Ok(false);
        }
        if !matches!(
            self.game.hand().map(SichuanHand::phase),
            Some(HandPhase::AwaitingExchangeAnimation)
        ) {
            return Ok(false);
        }
        if !self
            .exchange
            .release_if_due(now_ms, EXCHANGE_ANIMATION_FALLBACK_MS)
        {
            return Ok(false);
        }
        self.game
            .hand_mut()
            .ok_or_else(|| internal_error("there is no hand in progress"))?
            .advance_from_exchange_animation()
            .map_err(|error| internal_error(error.to_string()))?;
        self.begin_dingque_phase(now_ms);
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    fn request_exit_vote(&mut self, seat: u8, now_ms: u64) -> Result<(), ApplicationError> {
        if !self.friend_match {
            return Err(invalid_command(
                "exit votes are only available in friend matches",
            ));
        }
        if self.terminated_by_exit_vote || self.result.is_some() {
            return Err(invalid_command("match is already finished"));
        }
        if self.pending.is_some() {
            return Err(invalid_command(
                "exit vote cannot start during hand settlement",
            ));
        }
        if self.exit_vote.is_some() {
            return Err(invalid_command("an exit vote is already active"));
        }
        let index = usize::from(seat);
        if self.exit_vote_used_hand[index] == Some(self.hand_index) {
            return Err(invalid_command(
                "this player already started an exit vote in this hand",
            ));
        }
        self.exit_vote_used_hand[index] = Some(self.hand_index);
        let mut votes = vec![None; SEATS].into_boxed_slice();
        votes[index] = Some(true);
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
        seat: u8,
        agree: bool,
        now_ms: u64,
    ) -> Result<(), ApplicationError> {
        let vote = self
            .exit_vote
            .as_mut()
            .ok_or_else(|| invalid_command("there is no active exit vote"))?;
        let choice = &mut vote.votes[usize::from(seat)];
        if choice.is_some() {
            return Err(invalid_command("this player already voted"));
        }
        *choice = Some(agree);
        self.resolve_exit_vote(now_ms, false)
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
        let required = SEATS.div_ceil(2);
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
        let initiator = vote.initiator;
        let actor = self
            .players
            .iter()
            .find(|player| player.seat == initiator)
            .map(|player| player.user_id.clone())
            .ok_or_else(|| internal_error("exit vote initiator is missing"))?;
        self.resolve_exit_vote(now_ms, true)?;
        self.bump_version()?;
        Ok(Some(actor))
    }

    fn rearm_clocks(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        self.rearm_clocks_after(now_ms, 0)
    }

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
        if self.phase_deadline_ms.is_some() {
            for clock in &mut self.clocks {
                clock.disarm(now_ms);
            }
            return Ok(());
        }
        let start_ms = now_ms.saturating_add(grace_ms);
        for index in 0..SEATS {
            let seat = seat_of(u8::try_from(index).expect("seat count fits u8"))?;
            if self.is_waiting(seat) {
                self.clocks[index].arm(start_ms);
            } else {
                self.clocks[index].disarm(now_ms);
            }
        }
        Ok(())
    }

    fn is_waiting(&self, seat: Seat) -> bool {
        if self.pending.is_some() || self.result.is_some() {
            return false;
        }
        let Some(hand) = self.game.hand() else {
            return false;
        };
        let index = usize::from(seat.index());
        match hand.phase() {
            HandPhase::AwaitingExchange => !self.exchange.submitted_flags()[index],
            HandPhase::AwaitingExchangeAnimation => false,
            HandPhase::AwaitingDingQue => !self.dingque.submitted_flags()[index],
            HandPhase::AwaitingTurnAction { seat: waiting }
            | HandPhase::AwaitingDiscard { seat: waiting } => waiting == seat,
            HandPhase::AwaitingResponses { .. } => hand.pending_reactions().contains(&seat),
            HandPhase::AwaitingKanAnimation { .. }
            | HandPhase::AwaitingWinAnimation { .. }
            | HandPhase::Ended { .. } => false,
        }
    }

    /// 超时代打：
    /// - 换三张：自动挑同花色最多的三门，选三张交出去；
    /// - 定缺：自动挑张数最少的那门缺掉；
    /// - 等响应时能荣和就自动荣和，否则取消；
    /// - 轮到自己时能自摸就自动自摸，否则打摸上来那张，再不行就打最右边那张。
    fn timeout_command(&self, seat: Seat) -> Result<GameCommand, ApplicationError> {
        let hand = self
            .game
            .hand()
            .ok_or_else(|| internal_error("there is no hand in progress"))?;
        match hand.phase() {
            HandPhase::AwaitingExchange => {
                return Ok(GameCommand::SichuanExchange {
                    tile_ids: self.auto_exchange_tiles(seat)?,
                });
            }
            HandPhase::AwaitingDingQue => {
                return Ok(GameCommand::SichuanDingQue {
                    suit: self.auto_dingque_suit(seat)?.as_str().to_owned(),
                });
            }
            HandPhase::AwaitingResponses { .. } => {
                return Ok(if hand.reaction_options(seat).can_ron {
                    GameCommand::SichuanRon
                } else {
                    GameCommand::SichuanPass
                });
            }
            _ => {}
        }
        if hand.turn_actions(seat).can_tsumo {
            return Ok(GameCommand::SichuanTsumo);
        }
        let player = hand.player(seat);
        let candidates = player
            .drawn()
            .into_iter()
            .chain(player.concealed().iter().rev().map(|tile| tile.id()));
        for tile_id in candidates {
            let mut probe = hand.clone();
            if probe
                .apply_turn_action(seat, TurnAction::Discard { tile: tile_id })
                .is_ok()
            {
                return Ok(GameCommand::SichuanDiscard {
                    tile_id: tile_id.value(),
                });
            }
        }
        Err(internal_error("seat has no legal discard"))
    }

    fn auto_exchange_tiles(&self, seat: Seat) -> Result<[u16; 3], ApplicationError> {
        let hand = self
            .game
            .hand()
            .ok_or_else(|| internal_error("there is no hand in progress"))?;
        let player = hand.player(seat);
        let suit = most_frequent_suit(player, true).ok_or_else(|| internal_error("empty hand"))?;
        let ids = player
            .concealed()
            .iter()
            .filter(|tile| tile.kind().suit() == Some(suit))
            .take(3)
            .map(|tile| tile.id().value())
            .collect::<Vec<_>>();
        ids.try_into()
            .map_err(|_| internal_error("a hand always has three tiles of one suit"))
    }

    fn auto_dingque_suit(&self, seat: Seat) -> Result<Suit, ApplicationError> {
        let hand = self
            .game
            .hand()
            .ok_or_else(|| internal_error("there is no hand in progress"))?;
        let player = hand.player(seat);
        most_frequent_suit(player, false).ok_or_else(|| internal_error("empty hand"))
    }

    fn expire_phase_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        let Some(deadline) = self.phase_deadline_ms else {
            return Ok(false);
        };
        if now_ms < deadline {
            return Ok(false);
        }
        let Some(hand) = self.game.hand() else {
            self.phase_deadline_ms = None;
            return Ok(false);
        };
        let pending = match hand.phase() {
            HandPhase::AwaitingExchange => self
                .exchange
                .submitted_flags()
                .iter()
                .enumerate()
                .filter_map(|(index, submitted)| (!*submitted).then_some(index))
                .collect::<Vec<_>>(),
            HandPhase::AwaitingDingQue => self
                .dingque
                .submitted_flags()
                .iter()
                .enumerate()
                .filter_map(|(index, submitted)| (!*submitted).then_some(index))
                .collect::<Vec<_>>(),
            _ => {
                self.phase_deadline_ms = None;
                return Ok(false);
            }
        };
        for index in pending {
            let seat = u8::try_from(index).expect("seat count fits u8");
            let actor = self
                .players
                .iter()
                .find(|player| player.seat == seat)
                .map(|player| player.user_id.clone())
                .ok_or_else(|| internal_error("phase seat has no player"))?;
            let command = SubmitGameCommand {
                expected_version: self.version,
                command: self.timeout_command(seat_of(seat)?)?,
            };
            self.execute(&actor, command, now_ms)?;
        }
        self.phase_deadline_ms = None;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    pub(crate) fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        if let Some(actor) = self.advance_exit_vote_if_due(now_ms)? {
            return Ok(Some(actor));
        }
        if self.is_finished() || self.assets_loading() {
            return Ok(None);
        }
        if self.opening.opening_blocked() {
            return Ok(None);
        }
        if self.expire_phase_if_due(now_ms)? {
            return Ok(self.players.first().map(|player| player.user_id.clone()));
        }
        if self.release_exchange_if_due(now_ms)? {
            let actor = self
                .players
                .first()
                .map(|player| player.user_id.clone())
                .ok_or_else(|| internal_error("no players in match"))?;
            return Ok(Some(actor));
        }
        if self.advance_kan_animation_if_due(now_ms)? {
            let actor = self
                .players
                .first()
                .map(|player| player.user_id.clone())
                .ok_or_else(|| internal_error("no players in match"))?;
            return Ok(Some(actor));
        }
        if self.advance_win_animation_if_due(now_ms)? {
            return Ok(self.players.first().map(|player| player.user_id.clone()));
        }
        let Some(index) = (0..SEATS).find(|index| self.clocks[*index].expired(now_ms)) else {
            return Ok(None);
        };
        let seat = u8::try_from(index).expect("seat count fits u8");
        let actor = self
            .players
            .iter()
            .find(|player| player.seat == seat)
            .map(|player| player.user_id.clone())
            .ok_or_else(|| internal_error("expired seat has no player"))?;
        let command = SubmitGameCommand {
            expected_version: self.version,
            command: self.timeout_command(seat_of(seat)?)?,
        };
        self.execute(&actor, command, now_ms).inspect_err(|_| {
            self.clocks[index].disarm(now_ms);
        })?;
        Ok(Some(actor))
    }

    pub(crate) fn open_settlement_confirm_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.exit_vote.is_some() || self.terminated_by_exit_vote {
            return Ok(false);
        }
        let yaku_count = self.settlement_yaku_count();
        let Some(pending) = self.pending.as_mut() else {
            return Ok(false);
        };
        if !pending
            .flow
            .open_confirmation_if_due(now_ms, settlement_reveal_fallback_ms(yaku_count))
        {
            return Ok(false);
        }
        self.bump_version()?;
        Ok(true)
    }

    pub(crate) fn advance_settlement_if_due(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.exit_vote.is_some() || self.terminated_by_exit_vote {
            return Ok(false);
        }
        let yaku_count = self.settlement_yaku_count();
        let due = self.pending.as_ref().is_some_and(|pending| {
            pending
                .flow
                .advance_due(now_ms, settlement_fallback_ms(yaku_count))
        });
        if !due {
            return Ok(false);
        }
        self.advance_settlement(now_ms)?;
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    /// 结算摊牌动画的时长按胡家与役种条数算，人越多留的兜底越长。
    fn settlement_yaku_count(&self) -> usize {
        self.pending.as_ref().map_or(0, |pending| {
            pending
                .settlement
                .winners()
                .iter()
                .map(|winner| winner.evaluation().yaku().len())
                .sum()
        })
    }

    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        if self.is_finished() || self.assets_loading() {
            return Ok(false);
        }
        if !self.opening.release_opening_if_due(now_ms) {
            return Ok(false);
        }
        if matches!(
            self.game.hand().map(SichuanHand::phase),
            Some(HandPhase::AwaitingExchange)
        ) {
            self.begin_exchange_phase(now_ms);
        }
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

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
        self.bump_version()?;
        Ok(true)
    }

    #[must_use]
    pub(crate) fn is_finished(&self) -> bool {
        self.result.is_some()
            || self.terminated_by_exit_vote
            || self.opening.terminated_by_asset_timeout()
    }

    #[must_use]
    pub(crate) const fn has_pending_settlement(&self) -> bool {
        self.pending.is_some()
    }

    fn bump_version(&mut self) -> Result<(), ApplicationError> {
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| internal_error("match version overflow"))?;
        Ok(())
    }
}

fn seats_with_flag(flags: &[bool]) -> Vec<u8> {
    flags
        .iter()
        .enumerate()
        .filter(|(_, set)| **set)
        .map(|(index, _)| u8::try_from(index).expect("seat count fits u8"))
        .collect()
}

fn seat_of(index: u8) -> Result<Seat, ApplicationError> {
    Seat::new(index).map_err(|_| internal_error("seat index is out of range"))
}

fn tile_view(tile: mahjong_sichuan::Tile) -> SichuanTileView {
    SichuanTileView {
        id: tile.id().value(),
        code: tile.kind().to_string(),
    }
}

fn meld_view(meld: &mahjong_sichuan::Meld) -> SichuanMeldView {
    SichuanMeldView {
        id: meld.id().value(),
        kind: meld.kind().as_str(),
        tiles: meld.tiles().iter().copied().map(tile_view).collect(),
        called_from: meld.called_from().map(Seat::index),
        called_tile_id: meld.called_tile().map(TileId::value),
    }
}

fn drawn_tile_view(hand: &SichuanHand, seat: Seat) -> Option<SichuanTileView> {
    hand.player(seat)
        .drawn()
        .and_then(|id| {
            hand.player(seat)
                .concealed()
                .iter()
                .find(|tile| tile.id() == id)
                .copied()
        })
        .map(tile_view)
}

/// 从牌码解析定缺门。`Suit` 没有 `FromStr`，这里收编成一个小函数。
fn parse_suit(code: &str) -> Option<Suit> {
    match code {
        "man" => Some(Suit::Man),
        "pin" => Some(Suit::Pin),
        "sou" => Some(Suit::Sou),
        _ => None,
    }
}

/// 挑一门花色：`most` 为真取张数最多的（换三张），否则取最少的（定缺）。
fn most_frequent_suit(player: &mahjong_sichuan::PlayerHand, most: bool) -> Option<Suit> {
    let mut counts = [0_u8; 3];
    for tile in player.concealed() {
        if let Some(suit) = tile.kind().suit() {
            counts[suit.index()] += 1;
        }
    }
    let pick = if most {
        (0..3).max_by_key(|index| counts[*index])
    } else {
        (0..3).min_by_key(|index| counts[*index])
    }?;
    Some(Suit::ALL[pick])
}

fn invalid_command(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidGameCommand, error.to_string())
}

fn internal_error(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, message)
}
