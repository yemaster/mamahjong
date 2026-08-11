//! 冲击麻将的对局运行时。
//!
//! 与 `game.rs` 里的立直运行时**互不引用**：那一套一行都没动，这里从零搭一份
//! 会话流程（素材握手、开局动画放行、结算三段式、退出投票、读秒）。两边共用的
//! 只有 `SeatClock` 与 `presentation` 里的那几个时长常量。
//!
//! 投影一律用 `u8` 座位号与牌码字符串，DTO 层直接照抄即可，不必再认识引擎类型。

use mahjong_core::{MatchId, RoomId, UserId};
use mahjong_impact::{
    AllInKind, HandPhase, ImpactMatch, ImpactRuleSnapshot, ImpactRules, MeldKind, ReactionKind,
    Seat, TileId, TurnAction, WallSeed,
};

/// 杠完之后等四家播完杠点动画的挂起状态。动画最多约 2 s，加上宽限合计留 6 s 兜底。
const KAN_ANIMATION_FALLBACK_MS: u64 = 6_000;

use crate::clock::SeatClock;
use crate::game::{GameCommand, SubmitGameCommand, shuffle_players};
use crate::presentation::{
    ANIMATION_REPORT_GRACE_MS, MATCH_ASSET_LOAD_TIMEOUT_MS, OPENING_READY_FALLBACK_MS,
    SETTLEMENT_CONFIRM_MS, animation_grace_ms, settlement_fallback_ms,
    settlement_reveal_fallback_ms,
};
use crate::{ApplicationError, ErrorCode, Room, RoomLifecycle};

const EXIT_VOTE_DURATION_MS: u64 = 15_000;
const SEATS: usize = mahjong_impact::SEAT_COUNT as usize;

/// 一位在座玩家的身份，和立直那边的 `MatchPlayer` 同形，只是座位是 `u8`。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactPlayer {
    user_id: UserId,
    seat: u8,
    nickname: String,
}

impl ImpactPlayer {
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
pub struct ImpactTileView {
    pub id: u16,
    pub code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactMeldView {
    pub id: u16,
    pub kind: &'static str,
    pub tiles: Vec<ImpactTileView>,
    pub called_from: Option<u8>,
    pub called_tile_id: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactDiscardView {
    pub tile: ImpactTileView,
    pub called: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactPlayer {
    pub player: ImpactPlayer,
    pub points: i32,
    pub kan_points: i32,
    pub concealed_tile_count: usize,
    /// 只有本人（或摊牌时）看得到具体是哪几张。
    pub concealed_tiles: Option<Vec<ImpactTileView>>,
    pub drawn_tile_id: Option<u16>,
    pub melds: Vec<ImpactMeldView>,
    pub discards: Vec<ImpactDiscardView>,
    /// 本局已经成立的真杠数，三杠全交要看它。
    pub kan_count: u8,
    /// 连续打出的字牌 / 财神数，连打十一风要看它。
    pub honor_streak: u32,
}

/// 轮到自己时的合法选项。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImpactTurnActionsView {
    pub can_tsumo: bool,
    /// 可以暗杠的牌码。
    pub concealed_kans: Vec<String>,
    /// 可以加杠的副露编号。
    pub added_kans: Vec<u16>,
    pub indicator_concealed_kan: bool,
    /// 打哪张能听：(打出的牌 id, 打出后能和的牌码列表)。
    pub tenpai_discard_hints: Vec<(u16, Vec<String>)>,
}

/// 对别家打出的那张牌可以做的事。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImpactReactionOptionsView {
    pub can_pon: bool,
    pub can_open_kan: bool,
    pub pon_is_indicator: bool,
}

/// 第一巡连打的杠点，不是杠打出来的，但走同一个浮层。
pub(crate) const FIRST_ROUND_REPEAT_DISCARD: &str = "first_round_repeat_discard";

/// 一次杠点变动，供前端播那个不需要确认的浮层。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObserverKanPoints {
    /// 递增序号：客户端靠它认出「这是新的一次」。
    pub id: u64,
    /// 引发这次变动的座位；谁付谁收一律看 `deltas`，第一巡连打里这两者并不是同一个人。
    pub seat: u8,
    pub kind: &'static str,
    pub deltas: [i32; SEATS],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactSettlement {
    pub reason: &'static str,
    pub winner: Option<u8>,
    /// 全交时只报这一条，其余番种一律不列。
    pub all_in: Option<&'static str>,
    pub yaku: Vec<ObserverImpactYaku>,
    /// 全交时这里是 0，前端改写成「全交」。
    pub value: u32,
    pub point_deltas: [i32; SEATS],
    pub kan_point_deltas: [i32; SEATS],
    pub points_after: [i32; SEATS],
    pub kan_points_after: [i32; SEATS],
    /// 荒牌：本局不算，同一庄直接重开。
    pub void_hand: bool,
    pub played_seats: Vec<u8>,
    pub confirm_deadline_ms: Option<u64>,
    pub confirmed_seats: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactYaku {
    pub name: &'static str,
    pub value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactResult {
    pub final_points: [i32; SEATS],
    pub kan_points: [i32; SEATS],
    pub point_deltas: [i32; SEATS],
    /// 按剩余点数从高到低排的座位号。
    pub placements: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactExitVote {
    pub initiator: u8,
    pub deadline_ms: u64,
    pub votes: Vec<Option<bool>>,
}

/// 一位观察者看到的冲击麻将牌桌。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverImpactMatch {
    pub id: MatchId,
    pub room_id: RoomId,
    pub observer_seat: u8,
    pub version: u64,
    pub event_sequence: u64,
    pub hand_index: u32,
    pub dealer: u8,
    pub dealer_streak: u32,
    /// 本局用的整套规则，前端按它决定按钮文案与建房面板回显。
    pub rules: ImpactRules,
    pub phase_kind: &'static str,
    pub phase_seat: Option<u8>,
    pub phase_reason: Option<&'static str>,
    pub remaining_draws: usize,
    pub joker_indicator: Option<ImpactTileView>,
    /// 财神本身的牌码（指示牌的下一张）。
    pub joker_code: Option<String>,
    pub players: Vec<ObserverImpactPlayer>,
    pub reaction_options: ImpactReactionOptionsView,
    pub turn_actions: ImpactTurnActionsView,
    pub clocks: Vec<SeatClock>,
    pub opening_ready_seats: Vec<u8>,
    pub assets_ready_seats: Vec<u8>,
    pub terminated_by_asset_timeout: bool,
    pub hand_settlement: Option<ObserverImpactSettlement>,
    pub last_kan: Option<ObserverKanPoints>,
    pub result: Option<ObserverImpactResult>,
    pub friend_match: bool,
    pub can_start_exit_vote: bool,
    pub exit_vote: Option<ObserverImpactExitVote>,
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
    /// 对应 `last_kan.id`，用于客户端幂等报告。
    kan_id: u64,
    /// 动画播完后要摸岭上牌的座位。
    seat: Seat,
    started_at_ms: u64,
    played: [bool; SEATS],
}

#[derive(Clone, Debug)]
struct PendingSettlement {
    settlement: mahjong_impact::HandSettlement,
    /// 本局的庄家和连庄数。`settle_hand()` 当场就把庄挪走了，可结算画面还铺在
    /// 桌上，视图这时候要是跟着换庄，牌山、自风、庄标会当场跳一下。挪庄的结果先
    /// 存在这儿，等四家点完确认、开下一局再露出去。
    dealer: u8,
    dealer_streak: u32,
    settled_at_ms: u64,
    played: [bool; SEATS],
    first_played_at_ms: Option<u64>,
    confirm_started_at_ms: Option<u64>,
    confirmed: [bool; SEATS],
}

#[derive(Clone, Debug)]
pub(crate) struct ImpactRuntime {
    pub(crate) id: MatchId,
    pub(crate) room_id: RoomId,
    pub(crate) version: u64,
    /// 冲击麻将暂不生成事件流，游标恒为 0；客户端走视图订阅。
    pub(crate) event_sequence: u64,
    hand_index: u32,
    pub(crate) players: Box<[ImpactPlayer]>,
    pub(crate) rule_snapshot: ImpactRuleSnapshot,
    game: ImpactMatch,
    result: Option<ObserverImpactResult>,
    clocks: Box<[SeatClock]>,
    opening_ready: [bool; SEATS],
    assets_ready: [bool; SEATS],
    assets_started_at_ms: u64,
    terminated_by_asset_timeout: bool,
    opening_started_at_ms: u64,
    first_opening_ready_at_ms: Option<u64>,
    pending: Option<PendingSettlement>,
    pending_kan_animation: Option<PendingKanAnimation>,
    last_kan: Option<ObserverKanPoints>,
    kan_sequence: u64,
    pub(crate) friend_match: bool,
    exit_vote_used_hand: [Option<u32>; SEATS],
    exit_vote: Option<ExitVote>,
    terminated_by_exit_vote: bool,
}

impl ImpactRuntime {
    pub(crate) fn start(room: &Room, id: MatchId, now_ms: u64) -> Result<Self, ApplicationError> {
        if room.lifecycle() != RoomLifecycle::Playing || room.active_match_id() != Some(&id) {
            return Err(internal_error("room is not linked to the starting match"));
        }
        let snapshot = room
            .rule_snapshot()
            .as_impact()
            .ok_or_else(|| internal_error("room does not carry an impact rule snapshot"))?;
        let rules = *snapshot.rules();
        let thinking_time = rules.match_rules.thinking_time;
        let dealer = Seat::new(0).map_err(|_| internal_error("starting dealer is invalid"))?;
        let mut game = ImpactMatch::new(rules, dealer);
        let seed = WallSeed::generate().map_err(|error| internal_error(error.to_string()))?;
        game.start_hand(&seed)
            .map_err(|error| internal_error(error.to_string()))?;

        let mut members = room.members().iter().collect::<Vec<_>>();
        shuffle_players(&mut members)?;
        if members.len() != SEATS {
            return Err(internal_error("impact matches need exactly four players"));
        }
        let players = members
            .into_iter()
            .enumerate()
            .map(|(seat_index, member)| {
                Ok(ImpactPlayer {
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
            opening_ready: [false; SEATS],
            assets_ready: [false; SEATS],
            assets_started_at_ms: now_ms,
            terminated_by_asset_timeout: false,
            opening_started_at_ms: now_ms,
            first_opening_ready_at_ms: None,
            pending: None,
            pending_kan_animation: None,
            last_kan: None,
            kan_sequence: 0,
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

    fn assets_loading(&self) -> bool {
        self.assets_ready.iter().any(|ready| !*ready)
    }

    fn frozen(&self) -> bool {
        self.exit_vote.is_some()
            || self.terminated_by_exit_vote
            || self.assets_loading()
            || self.terminated_by_asset_timeout
    }

    pub(crate) fn view(&self, actor: &UserId) -> Result<ObserverImpactMatch, ApplicationError> {
        let observer_seat = self.seat(actor)?;
        let hand = self.game.hand();
        let revealed = self.pending.is_some() || self.result.is_some();
        // 结算动画期间，点数和杠点要维持上一局的值，动画才是从旧值滚到新值。
        let (base_points, base_kan_points): ([i32; SEATS], [i32; SEATS]) =
            if let Some(ref pending) = self.pending {
                let s = &pending.settlement;
                let mut pts = [0_i32; SEATS];
                let mut kpts = [0_i32; SEATS];
                for i in 0..SEATS {
                    pts[i] = s.points_after()[i] - s.point_deltas()[i];
                    kpts[i] = s.kan_points_after()[i] - s.kan_point_deltas()[i];
                }
                (pts, kpts)
            } else {
                (*self.game.points(), *self.game.kan_points())
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
                    honor_streak,
                ) = hand.map_or_else(
                    || (0, None, None, Vec::new(), Vec::new(), 0, 0),
                    |hand| {
                        let held = hand.player(seat);
                        let mine = player.seat == observer_seat;
                        (
                            held.concealed().len(),
                            (mine || revealed)
                                .then(|| held.concealed().iter().copied().map(tile_view).collect()),
                            (mine || revealed)
                                .then(|| held.drawn().map(TileId::value))
                                .flatten(),
                            held.melds().iter().map(meld_view).collect(),
                            held.discards()
                                .iter()
                                .map(|discard| ImpactDiscardView {
                                    tile: tile_view(discard.tile()),
                                    called: discard.called(),
                                })
                                .collect(),
                            held.kan_count(),
                            held.honor_streak(),
                        )
                    },
                );
                // 杠点分两处记：`ImpactMatch` 里是往局的总账，本局的增减挂在
                // `hand` 上，结算时才并进总账。杠完立刻要让四家看见新数字，所以
                // 局还没结束的时候把两边加起来；`outcome()` 一有值就说明已经并过
                // 账了，再加就成了双份。
                let live_kan_points = base_kan_points[index]
                    + hand
                        .filter(|hand| hand.outcome().is_none())
                        .map_or(0, |hand| hand.kan_point_deltas()[index]);
                Ok(ObserverImpactPlayer {
                    player: player.clone(),
                    points: base_points[index],
                    kan_points: live_kan_points,
                    concealed_tile_count,
                    concealed_tiles,
                    drawn_tile_id,
                    melds,
                    discards,
                    kan_count,
                    honor_streak,
                })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?;

        let observer = seat_of(observer_seat)?;
        let (phase_kind, phase_seat, phase_reason) =
            hand.map_or(("awaiting_turn_action", None, None), |hand| {
                match hand.phase() {
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
                    HandPhase::Ended { reason } => ("ended", None, Some(reason.as_str())),
                }
            });

        let (turn_actions, reaction_options) =
            if self.frozen() || self.pending.is_some() || self.pending_kan_animation.is_some() {
                (
                    ImpactTurnActionsView::default(),
                    ImpactReactionOptionsView::default(),
                )
            } else {
                hand.map_or_else(
                    || {
                        (
                            ImpactTurnActionsView::default(),
                            ImpactReactionOptionsView::default(),
                        )
                    },
                    |hand| {
                        let actions = hand.turn_actions(observer);
                        let options = hand.reaction_options(observer);
                        (
                            ImpactTurnActionsView {
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
                                indicator_concealed_kan: actions.indicator_concealed_kan,
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
                            ImpactReactionOptionsView {
                                can_pon: options.can_pon,
                                can_open_kan: options.can_open_kan,
                                pon_is_indicator: options.pon_is_indicator,
                            },
                        )
                    },
                )
            };

        Ok(ObserverImpactMatch {
            id: self.id.clone(),
            room_id: self.room_id.clone(),
            observer_seat,
            version: self.version,
            event_sequence: self.event_sequence,
            hand_index: self.hand_index,
            // 结算画面还在的时候报本局的庄，别让桌面提前换庄跳一下。
            dealer: self
                .pending
                .as_ref()
                .map_or_else(|| self.game.progress().dealer().index(), |p| p.dealer),
            dealer_streak: self.pending.as_ref().map_or_else(
                || self.game.progress().dealer_streak().value(),
                |p| p.dealer_streak,
            ),
            rules: *self.rule_snapshot.rules(),
            phase_kind,
            phase_seat,
            phase_reason,
            remaining_draws: hand.map_or(0, mahjong_impact::ImpactHand::remaining_draws),
            joker_indicator: hand.map(|hand| tile_view(hand.indicator())),
            joker_code: hand.map(|hand| hand.joker().to_string()),
            players,
            reaction_options,
            turn_actions,
            clocks: self.clocks.to_vec(),
            opening_ready_seats: seats_with_flag(&self.opening_ready),
            assets_ready_seats: seats_with_flag(&self.assets_ready),
            terminated_by_asset_timeout: self.terminated_by_asset_timeout,
            hand_settlement: self.pending.as_ref().map(|pending| {
                let settlement = &pending.settlement;
                let evaluation = settlement.evaluation();
                ObserverImpactSettlement {
                    reason: settlement.reason().as_str(),
                    winner: settlement.winner().map(Seat::index),
                    all_in: evaluation
                        .and_then(mahjong_impact::WinEvaluation::all_in)
                        .map(all_in_code),
                    yaku: evaluation.map_or_else(Vec::new, |evaluation| {
                        evaluation
                            .yaku()
                            .iter()
                            .map(|value| ObserverImpactYaku {
                                name: crate::naming::impact_yaku_name(value.yaku()),
                                value: value.points(),
                            })
                            .collect()
                    }),
                    value: evaluation.map_or(0, mahjong_impact::WinEvaluation::points),
                    point_deltas: *settlement.point_deltas(),
                    kan_point_deltas: *settlement.kan_point_deltas(),
                    points_after: *settlement.points_after(),
                    kan_points_after: *settlement.kan_points_after(),
                    void_hand: settlement.is_void(),
                    played_seats: seats_with_flag(&pending.played),
                    confirm_deadline_ms: pending
                        .confirm_started_at_ms
                        .map(|started_ms| started_ms.saturating_add(SETTLEMENT_CONFIRM_MS)),
                    confirmed_seats: seats_with_flag(&pending.confirmed),
                }
            }),
            last_kan: self.last_kan,
            result: self.result.clone(),
            friend_match: self.friend_match,
            can_start_exit_vote: self.friend_match
                && !self.assets_loading()
                && !self.terminated_by_asset_timeout
                && self.exit_vote.is_none()
                && self.exit_vote_used_hand[usize::from(observer_seat)] != Some(self.hand_index)
                && !self.terminated_by_exit_vote
                && self.pending.is_none()
                && self.pending_kan_animation.is_none()
                && self.result.is_none(),
            exit_vote: self.exit_vote.as_ref().map(|vote| ObserverImpactExitVote {
                initiator: vote.initiator,
                deadline_ms: vote.deadline_ms,
                votes: vote.votes.to_vec(),
            }),
            terminated_by_exit_vote: self.terminated_by_exit_vote,
        })
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
                | GameCommand::ImpactKanAnimationPlayed { .. }
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
        if self.terminated_by_asset_timeout {
            return Err(ApplicationError::new(
                ErrorCode::MatchFinished,
                "match was terminated while waiting for players to load",
            ));
        }
        if matches!(command.command, GameCommand::MatchAssetsReady) {
            if self.assets_ready[index] {
                return Ok(());
            }
            self.assets_ready[index] = true;
            if self.assets_ready.iter().all(|ready| *ready) {
                self.opening_started_at_ms = now_ms;
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
                if self.opening_ready[index] {
                    return Ok(());
                }
                self.opening_ready[index] = true;
                self.first_opening_ready_at_ms.get_or_insert(now_ms);
                if self.opening_ready.iter().all(|ready| *ready) {
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
                if pending.played[index] {
                    return Ok(());
                }
                pending.played[index] = true;
                pending.first_played_at_ms.get_or_insert(now_ms);
                if pending.played.iter().all(|played| *played) {
                    pending.confirm_started_at_ms.get_or_insert(now_ms);
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
                if pending.confirm_started_at_ms.is_none() {
                    return Err(invalid_command("the settlement is still being played"));
                }
                if pending.confirmed[index] {
                    return Ok(());
                }
                pending.confirmed[index] = true;
                if pending.confirmed.iter().all(|confirmed| *confirmed) {
                    self.advance_settlement(now_ms)?;
                    self.rearm_clocks(now_ms)?;
                }
                return self.bump_version();
            }
            GameCommand::ImpactKanAnimationPlayed { kan_id } => {
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
                    self.advance_kan_animation(now_ms)?;
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
        if self.opening_ready.iter().any(|ready| !*ready) {
            return Err(invalid_command("the opening deal is still being dealt"));
        }
        let grace_ms = animation_grace_ms(&command.command);
        let actor_seat = seat_of(seat)?;
        let kan_points_before = *self
            .game
            .hand()
            .ok_or_else(|| invalid_command("there is no hand in progress"))?
            .kan_point_deltas();
        let kan_kind = self.apply_hand_command(actor_seat, &command.command)?;
        let hand = self
            .game
            .hand_mut()
            .ok_or_else(|| internal_error("the hand vanished mid-command"))?;
        let kan_points_after = *hand.kan_point_deltas();
        let discard_kind = hand.take_discard_kan_kind();
        // 杠点动了就播浮层。第一巡连打和四张相同牌都是打牌打出来的，没有 `kan_kind`，
        // 但账一样要让四家看见，所以这里认的是「账变了没有」而不是「这一步是不是杠」。
        if kan_points_before != kan_points_after {
            self.kan_sequence = self
                .kan_sequence
                .checked_add(1)
                .ok_or_else(|| internal_error("kan sequence overflow"))?;
            let mut deltas = [0_i32; SEATS];
            for (slot, delta) in deltas.iter_mut().enumerate() {
                *delta = kan_points_after[slot] - kan_points_before[slot];
            }
            self.last_kan = Some(ObserverKanPoints {
                id: self.kan_sequence,
                seat,
                kind: kan_kind
                    .or(discard_kind)
                    .unwrap_or(FIRST_ROUND_REPEAT_DISCARD),
                deltas,
            });
        }
        if hand.outcome().is_some() {
            self.finish_hand(now_ms)?;
        } else if matches!(
            self.game.hand().map(mahjong_impact::ImpactHand::phase),
            Some(HandPhase::AwaitingKanAnimation { .. })
        ) {
            // 杠完还没结束局，把当前的 kan_sequence 记进挂起状态，等四家报告动画播完。
            let pending_seat = match self.game.hand().map(mahjong_impact::ImpactHand::phase) {
                Some(HandPhase::AwaitingKanAnimation { seat }) => seat,
                _ => unreachable!("checked above"),
            };
            self.pending_kan_animation = Some(PendingKanAnimation {
                kan_id: self.kan_sequence,
                seat: pending_seat,
                started_at_ms: now_ms,
                played: [false; SEATS],
            });
        }
        self.bump_version()?;
        self.clocks[index].disarm(now_ms);
        self.rearm_clocks_after(now_ms, grace_ms)
    }

    /// 把一条游戏指令交给引擎；返回值是「这一步是哪种杠」，用来播杠点浮层。
    fn apply_hand_command(
        &mut self,
        seat: Seat,
        command: &GameCommand,
    ) -> Result<Option<&'static str>, ApplicationError> {
        let hand = self
            .game
            .hand_mut()
            .ok_or_else(|| invalid_command("there is no hand in progress"))?;
        let kan_kind = match command {
            GameCommand::ImpactDiscard { tile_id } => {
                hand.apply_turn_action(
                    seat,
                    TurnAction::Discard {
                        tile: TileId::new(*tile_id),
                    },
                )
                .map_err(invalid_command)?;
                None
            }
            GameCommand::ImpactTsumo => {
                hand.apply_turn_action(seat, TurnAction::Tsumo)
                    .map_err(invalid_command)?;
                None
            }
            GameCommand::ImpactConcealedKan { tile_code } => {
                let tile = tile_code
                    .parse()
                    .map_err(|_| invalid_command("unknown tile code"))?;
                hand.apply_turn_action(seat, TurnAction::ConcealedKan { tile })
                    .map_err(invalid_command)?;
                Some(MeldKind::ConcealedKan.as_str())
            }
            GameCommand::ImpactAddedKan { meld_id } => {
                hand.apply_turn_action(
                    seat,
                    TurnAction::AddedKan {
                        meld: mahjong_impact::MeldId::new(*meld_id),
                    },
                )
                .map_err(invalid_command)?;
                Some(MeldKind::AddedKan.as_str())
            }
            GameCommand::ImpactIndicatorConcealedKan => {
                hand.apply_turn_action(seat, TurnAction::IndicatorConcealedKan)
                    .map_err(invalid_command)?;
                Some(MeldKind::IndicatorConcealed.as_str())
            }
            GameCommand::ImpactPon => {
                let indicator = hand.reaction_options(seat).pon_is_indicator;
                hand.apply_reaction(seat, ReactionKind::Pon)
                    .map_err(invalid_command)?;
                indicator.then_some(MeldKind::IndicatorPon.as_str())
            }
            GameCommand::ImpactOpenKan => {
                hand.apply_reaction(seat, ReactionKind::OpenKan)
                    .map_err(invalid_command)?;
                Some(MeldKind::OpenKan.as_str())
            }
            GameCommand::ImpactPass => {
                hand.apply_reaction(seat, ReactionKind::Pass)
                    .map_err(invalid_command)?;
                None
            }
            _ => {
                return Err(invalid_command(
                    "this command is not part of impact mahjong",
                ));
            }
        };
        Ok(kan_kind)
    }

    fn finish_hand(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        // 先把本局的庄记下来，`settle_hand()` 会当场把庄挪到下一家。
        let dealer = self.game.progress().dealer().index();
        let dealer_streak = self.game.progress().dealer_streak().value();
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
            let mut kan_points = [0_i32; SEATS];
            let mut point_deltas = [0_i32; SEATS];
            for result in &results {
                let index = usize::from(result.seat.index());
                final_points[index] = result.points;
                kan_points[index] = result.kan_points;
                point_deltas[index] = result.point_delta;
            }
            self.result = Some(ObserverImpactResult {
                final_points,
                kan_points,
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
            dealer_streak,
            settled_at_ms: now_ms,
            played: [false; SEATS],
            first_played_at_ms: None,
            confirm_started_at_ms: None,
            confirmed: [false; SEATS],
        });
        Ok(())
    }

    fn advance_settlement(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        self.pending
            .take()
            .ok_or_else(|| invalid_command("there is no hand settlement to advance"))?;
        // 安全起见：新的一局开始前清除残留的杠动画挂起状态。
        self.pending_kan_animation = None;
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
        self.opening_ready = [false; SEATS];
        self.opening_started_at_ms = now_ms;
        self.first_opening_ready_at_ms = None;
        self.last_kan = None;
        Ok(())
    }

    /// 四家都报告动画播完（或兜底超时后）摸岭上牌。
    fn advance_kan_animation(&mut self, now_ms: u64) -> Result<(), ApplicationError> {
        let pending = self
            .pending_kan_animation
            .take()
            .ok_or_else(|| internal_error("advance_kan_animation called with no pending"))?;
        let hand = self
            .game
            .hand_mut()
            .ok_or_else(|| internal_error("hand vanished during kan animation"))?;
        hand.advance_from_kan_animation(pending.seat)
            .map_err(|error| internal_error(error.to_string()))?;
        // 摸牌后如果立即结局（最后一张岭上牌摸到了），需要走结算流程。
        if self.game.hand().is_some_and(|h| h.outcome().is_some()) {
            self.finish_hand(now_ms)?;
        }
        Ok(())
    }

    /// 杠点动画兜底：超时后替全家报告完成，强制摸岭上牌。
    fn advance_kan_animation_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        let due = self
            .pending_kan_animation
            .as_ref()
            .is_some_and(|p| now_ms.saturating_sub(p.started_at_ms) >= KAN_ANIMATION_FALLBACK_MS);
        if !due {
            return Ok(false);
        }
        self.advance_kan_animation(now_ms)?;
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
            || self.terminated_by_asset_timeout
            || self.opening_ready.iter().any(|ready| !*ready)
        {
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
        match hand.phase() {
            HandPhase::AwaitingTurnAction { seat: waiting }
            | HandPhase::AwaitingDiscard { seat: waiting } => waiting == seat,
            HandPhase::AwaitingResponses { .. } => hand.pending_reactions().contains(&seat),
            HandPhase::AwaitingKanAnimation { .. } | HandPhase::Ended { .. } => false,
        }
    }

    fn opening_ready_deadline_passed(&self, now_ms: u64) -> bool {
        let deadline = match self.first_opening_ready_at_ms {
            Some(first_ready_ms) => first_ready_ms.saturating_add(ANIMATION_REPORT_GRACE_MS),
            None => self
                .opening_started_at_ms
                .saturating_add(OPENING_READY_FALLBACK_MS),
        };
        now_ms >= deadline
    }

    /// 超时代打：等响应就 Pass，轮到自己就打摸上来那张，再不行就打最右边那张。
    fn timeout_command(&self, seat: Seat) -> Result<GameCommand, ApplicationError> {
        let hand = self
            .game
            .hand()
            .ok_or_else(|| internal_error("there is no hand in progress"))?;
        if matches!(hand.phase(), HandPhase::AwaitingResponses { .. }) {
            return Ok(GameCommand::ImpactPass);
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
                return Ok(GameCommand::ImpactDiscard {
                    tile_id: tile_id.value(),
                });
            }
        }
        Err(internal_error("seat has no legal discard"))
    }

    pub(crate) fn expire(&mut self, now_ms: u64) -> Result<Option<UserId>, ApplicationError> {
        if let Some(actor) = self.advance_exit_vote_if_due(now_ms)? {
            return Ok(Some(actor));
        }
        if self.is_finished() || self.assets_loading() {
            return Ok(None);
        }
        if self.opening_ready.iter().any(|ready| !*ready) {
            return Ok(None);
        }
        // 杠点动画兜底超时。
        if self.advance_kan_animation_if_due(now_ms)? {
            return Ok(None);
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
        let Some(pending) = self.pending.as_mut() else {
            return Ok(false);
        };
        if pending.confirm_started_at_ms.is_some() {
            return Ok(false);
        }
        // 确认窗口只在「全场都报告播完」或「兜底到期」两个时刻打开。
        // 不设短宽限：结算动画时长随役种多少波动很大，一家早到不该替全场抢跑。
        let yaku_count: usize = pending
            .settlement
            .evaluation()
            .map(|e| e.yaku().len())
            .unwrap_or(0);
        let deadline_ms = pending
            .settled_at_ms
            .saturating_add(settlement_reveal_fallback_ms(yaku_count));
        if now_ms < deadline_ms {
            return Ok(false);
        }
        pending.played.fill(true);
        pending.confirm_started_at_ms = Some(now_ms);
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
        let due = self.pending.as_ref().is_some_and(|pending| {
            let confirm_due = pending.confirm_started_at_ms.is_some_and(|started_ms| {
                now_ms.saturating_sub(started_ms) >= SETTLEMENT_CONFIRM_MS
            });
            let yaku_count: usize = pending
                .settlement
                .evaluation()
                .map(|e| e.yaku().len())
                .unwrap_or(0);
            confirm_due
                || now_ms.saturating_sub(pending.settled_at_ms)
                    >= settlement_fallback_ms(yaku_count)
        });
        if !due {
            return Ok(false);
        }
        self.advance_settlement(now_ms)?;
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    pub(crate) fn release_opening_if_due(&mut self, now_ms: u64) -> Result<bool, ApplicationError> {
        if self.is_finished() || self.assets_loading() {
            return Ok(false);
        }
        if self.opening_ready.iter().all(|ready| *ready)
            || !self.opening_ready_deadline_passed(now_ms)
        {
            return Ok(false);
        }
        self.opening_ready = [true; SEATS];
        self.bump_version()?;
        self.rearm_clocks(now_ms)?;
        Ok(true)
    }

    pub(crate) fn terminate_if_assets_stalled(
        &mut self,
        now_ms: u64,
    ) -> Result<bool, ApplicationError> {
        if self.is_finished() || !self.assets_loading() {
            return Ok(false);
        }
        if now_ms.saturating_sub(self.assets_started_at_ms) < MATCH_ASSET_LOAD_TIMEOUT_MS {
            return Ok(false);
        }
        self.terminated_by_asset_timeout = true;
        for clock in &mut self.clocks {
            clock.disarm(now_ms);
        }
        self.bump_version()?;
        Ok(true)
    }

    #[must_use]
    pub(crate) const fn is_finished(&self) -> bool {
        self.result.is_some() || self.terminated_by_exit_vote || self.terminated_by_asset_timeout
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

fn tile_view(tile: mahjong_impact::Tile) -> ImpactTileView {
    ImpactTileView {
        id: tile.id().value(),
        code: tile.kind().to_string(),
    }
}

fn meld_view(meld: &mahjong_impact::Meld) -> ImpactMeldView {
    ImpactMeldView {
        id: meld.id().value(),
        kind: meld.kind().as_str(),
        tiles: meld.tiles().iter().copied().map(tile_view).collect(),
        called_from: meld.called_from().map(Seat::index),
        called_tile_id: meld.called_tile().map(TileId::value),
    }
}

const fn all_in_code(kind: AllInKind) -> &'static str {
    kind.as_str()
}

fn invalid_command(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidGameCommand, error.to_string())
}

fn internal_error(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, message)
}
