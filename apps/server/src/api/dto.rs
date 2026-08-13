use mahjong_riichi::{
    Discard, HandPhase, MatchResult, Meld, MeldKind, Reaction, RiichiStatus, Tile,
};
use mamahjong_application::{
    AccountRole, AccountStatus, Application, Character, CharacterSummary, DiscardWaitHint,
    GameRuleSnapshot, ImpactDiscardView, ImpactMeldView, ImpactTileView, MatchProjection,
    ObserverImpactMatch, ObserverImpactPlayer, ObserverMatch, ObserverPlayer, RankSummary, Room,
    RoomLifecycle, RoomMember, RoomVisibility, Session, TitleSummary, User, UserProfile,
    end_reason_name, impact_rule_display_name, limit_name, rule_display_name, wind_name, yaku_name,
};
use serde::Serialize;

use crate::clock::SeatCountdown;

use super::error::ApiError;

#[derive(Serialize)]
pub(super) struct AuthResponse {
    pub(super) user: UserResponse,
    pub(super) session: SessionResponse,
}

impl AuthResponse {
    pub(super) fn new(user: &User, session: &Session) -> Self {
        Self {
            user: UserResponse::from(user),
            session: SessionResponse {
                id: session.id().as_str().to_owned(),
                token: session.token().to_owned(),
                token_type: "Bearer",
            },
        }
    }
}

#[derive(Serialize)]
pub(super) struct SessionResponse {
    id: String,
    token: String,
    token_type: &'static str,
}

#[derive(Serialize)]
pub(super) struct UserResponse {
    id: String,
    version: u64,
    login_name: String,
    status: &'static str,
    role: &'static str,
    profile: ProfileResponse,
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        Self {
            id: user.id().as_str().to_owned(),
            version: user.version(),
            login_name: user.login_name().to_owned(),
            status: match user.status() {
                AccountStatus::Active => "active",
                AccountStatus::Suspended => "suspended",
            },
            role: match user.role() {
                AccountRole::Player => "player",
                AccountRole::Administrator => "administrator",
            },
            profile: ProfileResponse::from(user.profile()),
        }
    }
}

#[derive(Serialize)]
pub(super) struct ProfileResponse {
    nickname: String,
    equipped_title: Option<TitleResponse>,
    selected_character: Option<CharacterResponse>,
    selected_outfit_id: Option<String>,
    avatar_path: Option<String>,
    selected_tablecloth_id: Option<String>,
    selected_lobby_music_id: Option<String>,
    selected_match_music_id: Option<String>,
    selected_riichi_music_id: Option<String>,
    ranks: Vec<RankResponse>,
}

impl From<&UserProfile> for ProfileResponse {
    fn from(profile: &UserProfile) -> Self {
        Self {
            nickname: profile.nickname().as_str().to_owned(),
            equipped_title: profile.equipped_title().map(TitleResponse::from),
            selected_character: profile.selected_character().map(CharacterResponse::from),
            selected_outfit_id: profile.selected_outfit_id().map(str::to_owned),
            avatar_path: profile.avatar_path().map(str::to_owned),
            selected_tablecloth_id: profile.selected_tablecloth_id().map(str::to_owned),
            selected_lobby_music_id: profile.selected_lobby_music_id().map(str::to_owned),
            selected_match_music_id: profile.selected_match_music_id().map(str::to_owned),
            selected_riichi_music_id: profile.selected_riichi_music_id().map(str::to_owned),
            ranks: profile.ranks().iter().map(RankResponse::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct TitleResponse {
    id: String,
    name: String,
}

impl From<&TitleSummary> for TitleResponse {
    fn from(value: &TitleSummary) -> Self {
        Self {
            id: value.id().to_owned(),
            name: value.name().to_owned(),
        }
    }
}

#[derive(Serialize)]
struct CharacterResponse {
    id: String,
    name: String,
}

impl From<&CharacterSummary> for CharacterResponse {
    fn from(value: &CharacterSummary) -> Self {
        Self {
            id: value.id().to_owned(),
            name: value.name().to_owned(),
        }
    }
}

#[derive(Serialize)]
struct RankResponse {
    rule_set_id: String,
    queue_id: String,
    rank: String,
    points: i32,
}

impl From<&RankSummary> for RankResponse {
    fn from(value: &RankSummary) -> Self {
        Self {
            rule_set_id: value.rule_set_id().to_owned(),
            queue_id: value.queue_id().to_owned(),
            rank: value.rank().to_owned(),
            points: value.points(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct RoomListResponse {
    schema: &'static str,
    rooms: Vec<RoomResponse>,
}

impl RoomListResponse {
    pub(super) fn new(rooms: &[Room], application: &Application) -> Result<Self, ApiError> {
        let default_character = application.default_character()?;
        let characters = application.list_characters()?;
        Ok(Self {
            schema: "room_list.v1",
            rooms: rooms
                .iter()
                .map(|room| {
                    RoomResponse::new_with_characters(
                        room,
                        application,
                        &default_character,
                        &characters,
                    )
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize)]
pub(super) struct RoomResponse {
    schema: &'static str,
    id: String,
    version: u64,
    owner_user_id: String,
    name: String,
    visibility: &'static str,
    lifecycle: &'static str,
    seat_count: u8,
    /// 规则家族。前端照它挑设置面板与牌桌渲染分支。
    variant_kind: &'static str,
    /// 规则名：挂着预设写预设短名（「A规」「ML规则」），改过或自己捏的写「自定义规则」。
    ///
    /// 光有 `rule_snapshot` 认不出这个——得拿配置跟预设逐项比，那份比对逻辑在
    /// `mamahjong-application` 里，前端没有预设表，所以由服务端算好挂上来。
    rule_name: &'static str,
    rule_snapshot: serde_json::Value,
    members: Vec<RoomMemberResponse>,
    active_match_id: Option<String>,
}

impl RoomResponse {
    pub(super) fn new(room: &Room, application: &Application) -> Result<Self, ApiError> {
        let default_character = application.default_character()?;
        let characters = application.list_characters()?;
        Self::new_with_characters(room, application, &default_character, &characters)
    }

    fn new_with_characters(
        room: &Room,
        application: &Application,
        default_character: &Character,
        characters: &[Character],
    ) -> Result<Self, ApiError> {
        let (rule_snapshot, rule_name) = match room.rule_snapshot() {
            GameRuleSnapshot::Riichi(snapshot) => (
                serde_json::to_value(snapshot).map_err(|_| ApiError::internal())?,
                rule_display_name(snapshot),
            ),
            GameRuleSnapshot::Impact(snapshot) => (
                serde_json::to_value(snapshot).map_err(|_| ApiError::internal())?,
                impact_rule_display_name(snapshot),
            ),
        };
        Ok(Self {
            schema: "room.v1",
            id: room.id().as_str().to_owned(),
            version: room.version(),
            owner_user_id: room.owner_user_id().as_str().to_owned(),
            name: room.name().to_owned(),
            visibility: match room.visibility() {
                RoomVisibility::Public => "public",
                RoomVisibility::Private => "private",
            },
            lifecycle: match room.lifecycle() {
                RoomLifecycle::Waiting => "waiting",
                RoomLifecycle::Playing => "playing",
                RoomLifecycle::Closed => "closed",
            },
            seat_count: room.rule_snapshot().seat_count(),
            variant_kind: room.rule_snapshot().variant_kind(),
            rule_name,
            rule_snapshot,
            members: room
                .members()
                .iter()
                .map(|member| {
                    RoomMemberResponse::new(member, application, default_character, characters)
                })
                .collect::<Result<_, _>>()?,
            active_match_id: room
                .active_match_id()
                .map(|match_id| match_id.as_str().to_owned()),
        })
    }
}

#[derive(Serialize)]
struct RoomMemberResponse {
    user_id: String,
    seat: u8,
    nickname: String,
    ready: bool,
    character: RoomMemberCharacterResponse,
}

impl RoomMemberResponse {
    fn new(
        member: &RoomMember,
        application: &Application,
        default_character: &Character,
        characters: &[Character],
    ) -> Result<Self, ApiError> {
        let user = application.user(member.user_id())?;
        let selected_character = user.profile().selected_character();
        let character = selected_character
            .and_then(|selected| {
                characters
                    .iter()
                    .find(|character| character.id() == selected.id() && character.enabled())
            })
            .unwrap_or(default_character);
        Ok(Self {
            user_id: member.user_id().as_str().to_owned(),
            seat: member.seat(),
            nickname: member.nickname().to_owned(),
            ready: member.ready(),
            character: RoomMemberCharacterResponse {
                id: character.id().to_owned(),
                name: character.name().to_owned(),
                illustration_path: selected_illustration_path(&user, character),
            },
        })
    }
}

#[derive(Serialize)]
struct RoomMemberCharacterResponse {
    id: String,
    name: String,
    illustration_path: String,
}

#[derive(Serialize)]
pub(super) struct StartRoomResponse {
    pub(super) schema: &'static str,
    pub(super) match_id: String,
    pub(super) room: RoomResponse,
}

/// 一张牌桌的完整视图。
///
/// 立直麻将与冲击麻将共用这一个 `match_view.v1` 形状：两边都有的字段照常填，
/// 只属于一边的字段是 `Option` 且**为空时不进 JSON**，因此立直那条路径吐出来的
/// 字节和加冲击麻将之前完全一致（`variant_kind` 除外，它是新的判别字段）。
#[derive(Serialize)]
pub(super) struct MatchViewResponse {
    schema: &'static str,
    /// `"riichi"` 或 `"impact"`：前端先读它，再决定后面那些可选字段该不该看。
    variant_kind: &'static str,
    id: String,
    room_id: String,
    version: u64,
    event_sequence: u64,
    hand_index: u32,
    observer_seat: u8,
    progress: ProgressResponse,
    phase: PhaseResponse,
    remaining_live_draws: usize,
    dora_indicators: Vec<TileResponse>,
    /// 冲击麻将的财神指示牌。左上角只画这一张。
    #[serde(skip_serializing_if = "Option::is_none")]
    joker_indicator: Option<TileResponse>,
    /// 指示牌推出来的财神牌码。手牌里凡是这个牌码的都当百搭。
    #[serde(skip_serializing_if = "Option::is_none")]
    joker_code: Option<String>,
    /// 连庄次数。冲击麻将用它替掉「东一局」。
    #[serde(skip_serializing_if = "Option::is_none")]
    dealer_streak: Option<u32>,
    /// 这局生效的冲击麻将规则，供前端标注按钮与帮助文案。
    #[serde(skip_serializing_if = "Option::is_none")]
    impact_rules: Option<serde_json::Value>,
    players: Vec<MatchPlayerResponse>,
    available_reactions: Vec<ReactionResponse>,
    turn_actions: TurnActionsResponse,
    clocks: Vec<SeatCountdown>,
    opening_ready_seats: Vec<u8>,
    assets_ready_seats: Vec<u8>,
    terminated_by_asset_timeout: bool,
    hand_settlement: Option<HandSettlementResponse>,
    /// 最近一次杠点变动。前端照它放一次浮层，播完自动消失。
    #[serde(skip_serializing_if = "Option::is_none")]
    last_kan: Option<KanPointsResponse>,
    result: Option<MatchResultResponse>,
    friend_match: bool,
    can_start_exit_vote: bool,
    exit_vote: Option<ExitVoteResponse>,
    terminated_by_exit_vote: bool,
}

impl MatchViewResponse {
    pub(super) fn from_projection(
        value: &MatchProjection,
        now_ms: u64,
        application: &Application,
    ) -> Self {
        match value {
            MatchProjection::Riichi(view) => Self::new(view, now_ms, application),
            MatchProjection::Impact(view) => Self::impact(view, now_ms, application),
        }
    }

    pub(super) fn new(value: &ObserverMatch, now_ms: u64, application: &Application) -> Self {
        let progress = value.progress();
        Self {
            schema: "match_view.v1",
            variant_kind: "riichi",
            joker_indicator: None,
            joker_code: None,
            dealer_streak: None,
            impact_rules: None,
            last_kan: None,
            id: value.id().as_str().to_owned(),
            room_id: value.room_id().as_str().to_owned(),
            version: value.version(),
            event_sequence: value.event_sequence(),
            hand_index: value.hand_index(),
            observer_seat: value.observer_seat().index(),
            progress: ProgressResponse {
                round_wind: wind_name(progress.round_wind()),
                round_number: progress.round_number().value(),
                dealer: progress.dealer().index(),
                honba: progress.honba().value(),
                riichi_sticks: progress.riichi_sticks().value(),
            },
            phase: PhaseResponse::from(value.phase()),
            remaining_live_draws: value.remaining_live_draws(),
            dora_indicators: value
                .dora_indicators()
                .iter()
                .copied()
                .map(TileResponse::from)
                .collect(),
            players: value
                .players()
                .iter()
                .map(|player| MatchPlayerResponse::new(player, application))
                .collect(),
            available_reactions: value
                .available_reactions()
                .iter()
                .map(ReactionResponse::from)
                .collect(),
            turn_actions: TurnActionsResponse {
                can_tsumo: value.turn_actions().can_tsumo(),
                riichi_discard_tile_ids: value.turn_actions().riichi_discard_tile_ids().to_vec(),
                riichi_discard_hints: value
                    .turn_actions()
                    .riichi_discard_hints()
                    .iter()
                    .map(DiscardWaitHintResponse::from)
                    .collect(),
                tenpai_discard_hints: value
                    .turn_actions()
                    .tenpai_discard_hints()
                    .iter()
                    .map(DiscardWaitHintResponse::from)
                    .collect(),
                concealed_kan_tile_ids: value.turn_actions().concealed_kan_tile_ids().to_vec(),
                added_kan_options: value
                    .turn_actions()
                    .added_kan_options()
                    .iter()
                    .map(|option| AddedKanOptionResponse {
                        meld_id: option.meld_id(),
                        tile_id: option.tile_id(),
                    })
                    .collect(),
                can_nine_terminals: value.turn_actions().can_nine_terminals(),
                impact_concealed_kan_tile_codes: None,
                impact_added_kan_meld_ids: None,
                impact_indicator_concealed_kan: None,
            },
            clocks: SeatCountdown::snapshot(value.clocks(), now_ms),
            opening_ready_seats: value
                .opening_ready_seats()
                .map(mahjong_riichi::Seat::index)
                .collect(),
            assets_ready_seats: value
                .assets_ready_seats()
                .map(mahjong_riichi::Seat::index)
                .collect(),
            terminated_by_asset_timeout: value.terminated_by_asset_timeout(),
            hand_settlement: value
                .hand_settlement()
                .map(|settlement| HandSettlementResponse {
                    reason: end_reason_name(settlement.reason()),
                    tenpai_seats: settlement
                        .tenpai()
                        .iter()
                        .map(|seat| seat.index())
                        .collect(),
                    point_deltas: settlement.point_deltas().to_vec(),
                    points_before: settlement.points_before().to_vec(),
                    points_after: settlement.points_after().to_vec(),
                    winners: settlement
                        .winners()
                        .iter()
                        .map(|winner| {
                            let evaluation = winner.evaluation();
                            let mut yaku = evaluation
                                .yaku()
                                .iter()
                                .map(|value| YakuResponse {
                                    name: yaku_name(value.yaku()),
                                    value: u32::from(value.value()),
                                    yakuman: value.is_yakuman(),
                                })
                                .collect::<Vec<_>>();
                            if evaluation.yakuman_multiplier() == 0 {
                                let bonuses = evaluation.bonuses();
                                for (name, value) in [
                                    ("宝牌", bonuses.dora()),
                                    ("里宝牌", bonuses.ura_dora()),
                                    ("赤宝牌", bonuses.red_dora()),
                                ] {
                                    if value > 0 {
                                        yaku.push(YakuResponse {
                                            name,
                                            value: u32::from(value),
                                            yakuman: false,
                                        });
                                    }
                                }
                            }
                            WinnerSettlementResponse {
                                seat: winner.seat().index(),
                                han: evaluation.han(),
                                fu: evaluation.fu(),
                                yakuman_multiplier: evaluation.yakuman_multiplier(),
                                limit: limit_name(evaluation.limit()),
                                points: winner.points(),
                                dealer: winner.is_dealer(),
                                yaku,
                            }
                        })
                        .collect(),
                    played_seats: settlement
                        .played_seats()
                        .iter()
                        .map(|seat| seat.index())
                        .collect(),
                    confirm_remaining_ms: settlement
                        .confirm_deadline_ms()
                        .map(|deadline_ms| deadline_ms.saturating_sub(now_ms)),
                    confirmed_seats: settlement
                        .confirmed_seats()
                        .iter()
                        .map(|seat| seat.index())
                        .collect(),
                    from_seat: settlement.from().map(mahjong_riichi::Seat::index),
                    ura_dora_indicators: settlement
                        .ura_dora_indicators()
                        .iter()
                        .copied()
                        .map(TileResponse::from)
                        .collect(),
                    all_in: None,
                    kan_point_deltas: None,
                    kan_points_after: None,
                    void_hand: None,
                }),
            result: value.result().map(MatchResultResponse::from),
            friend_match: value.is_friend_match(),
            can_start_exit_vote: value.can_start_exit_vote(),
            exit_vote: value.exit_vote().map(|vote| ExitVoteResponse {
                initiator_seat: vote.initiator().index(),
                remaining_ms: vote.deadline_ms().saturating_sub(now_ms),
                votes: vote.votes().to_vec(),
            }),
            terminated_by_exit_vote: value.terminated_by_exit_vote(),
        }
    }

    /// 冲击麻将的牌桌。
    ///
    /// 立直独有的那些位置（场风本场、宝牌、立直状态、听牌提示、振听）在这里全部
    /// 填成空值：冲击麻将没有这些概念，前端的 impact 分支也不读它们。
    fn impact(value: &ObserverImpactMatch, now_ms: u64, application: &Application) -> Self {
        Self {
            schema: "match_view.v1",
            variant_kind: "impact",
            id: value.id.as_str().to_owned(),
            room_id: value.room_id.as_str().to_owned(),
            version: value.version,
            event_sequence: value.event_sequence,
            hand_index: value.hand_index,
            observer_seat: value.observer_seat,
            // 冲击麻将没有场风与本场，只有庄家和连庄次数，后者单独走 `dealer_streak`。
            progress: ProgressResponse {
                round_wind: "east",
                round_number: 1,
                dealer: value.dealer,
                honba: 0,
                riichi_sticks: 0,
            },
            phase: PhaseResponse::impact(value.phase_kind, value.phase_seat, value.phase_reason),
            remaining_live_draws: value.remaining_draws,
            dora_indicators: Vec::new(),
            joker_indicator: value.joker_indicator.as_ref().map(TileResponse::from),
            joker_code: value.joker_code.clone(),
            dealer_streak: Some(value.dealer_streak),
            impact_rules: serde_json::to_value(value.rules).ok(),
            players: value
                .players
                .iter()
                .map(|player| MatchPlayerResponse::impact(player, application))
                .collect(),
            available_reactions: ReactionResponse::impact(&value.reaction_options),
            turn_actions: TurnActionsResponse {
                can_tsumo: value.turn_actions.can_tsumo,
                riichi_discard_tile_ids: Vec::new(),
                riichi_discard_hints: Vec::new(),
                tenpai_discard_hints: value
                    .turn_actions
                    .tenpai_discard_hints
                    .iter()
                    .map(|(tile_id, codes)| DiscardWaitHintResponse {
                        tile_id: *tile_id,
                        waiting_tiles: codes
                            .iter()
                            .map(|code| WaitingTileResponse {
                                code: code.clone(),
                                // 冲击麻将每一手完整牌型都有分，有无财神都成立。
                                has_yaku: true,
                            })
                            .collect(),
                    })
                    .collect(),
                concealed_kan_tile_ids: Vec::new(),
                added_kan_options: Vec::new(),
                can_nine_terminals: false,
                impact_concealed_kan_tile_codes: Some(value.turn_actions.concealed_kans.clone()),
                impact_added_kan_meld_ids: Some(value.turn_actions.added_kans.clone()),
                impact_indicator_concealed_kan: Some(value.turn_actions.indicator_concealed_kan),
            },
            clocks: SeatCountdown::snapshot(&value.clocks, now_ms),
            opening_ready_seats: value.opening_ready_seats.clone(),
            assets_ready_seats: value.assets_ready_seats.clone(),
            terminated_by_asset_timeout: value.terminated_by_asset_timeout,
            hand_settlement: value.hand_settlement.as_ref().map(|settlement| {
                HandSettlementResponse {
                    reason: settlement.reason,
                    tenpai_seats: Vec::new(),
                    point_deltas: settlement.point_deltas.to_vec(),
                    // 结算前的点数就是结算后减去增减，省得再往投影里塞一份。
                    points_before: settlement
                        .points_after
                        .iter()
                        .zip(settlement.point_deltas.iter())
                        .map(|(after, delta)| after - delta)
                        .collect(),
                    points_after: settlement.points_after.to_vec(),
                    winners: settlement
                        .winner
                        .map(|seat| {
                            // 冲击麻将的日式 yaku 列表默认不包含底和（12 点），
                            // 这里在前面补上一条，前端算总和时不重复计。
                            let mut yaku: Vec<YakuResponse> = settlement
                                .yaku
                                .iter()
                                .map(|entry| YakuResponse {
                                    name: entry.name,
                                    value: entry.value,
                                    yakuman: false,
                                })
                                .collect();
                            let non_all_in = settlement.all_in.is_none() && settlement.value > 0;
                            if non_all_in {
                                yaku.insert(
                                    0,
                                    YakuResponse {
                                        name: "底和",
                                        value: mahjong_impact::BASE_VALUE,
                                        yakuman: false,
                                    },
                                );
                            }
                            WinnerSettlementResponse {
                                seat,
                                han: 0,
                                fu: 0,
                                yakuman_multiplier: 0,
                                limit: "",
                                points: settlement.value,
                                dealer: seat == value.dealer,
                                yaku,
                            }
                        })
                        .into_iter()
                        .collect(),
                    played_seats: settlement.played_seats.clone(),
                    confirm_remaining_ms: settlement
                        .confirm_deadline_ms
                        .map(|deadline_ms| deadline_ms.saturating_sub(now_ms)),
                    confirmed_seats: settlement.confirmed_seats.clone(),
                    from_seat: None,
                    ura_dora_indicators: Vec::new(),
                    all_in: settlement.all_in,
                    kan_point_deltas: Some(settlement.kan_point_deltas.to_vec()),
                    kan_points_after: Some(settlement.kan_points_after.to_vec()),
                    void_hand: Some(settlement.void_hand),
                }
            }),
            last_kan: value.last_kan.as_ref().map(|kan| KanPointsResponse {
                id: kan.id,
                seat: kan.seat,
                kind: kan.kind,
                deltas: kan.deltas.to_vec(),
            }),
            result: value.result.as_ref().map(|result| MatchResultResponse {
                end_reason: "points_exhausted",
                hand_count: value.hand_index,
                final_points: result.final_points.to_vec(),
                placements: result
                    .placements
                    .iter()
                    .enumerate()
                    .map(|(rank, seat)| PlacementResponse {
                        seat: *seat,
                        rank: u8::try_from(rank + 1).unwrap_or(u8::MAX),
                        points: result
                            .final_points
                            .get(usize::from(*seat))
                            .copied()
                            .unwrap_or_default(),
                        uma_tenths: 0,
                        oka_tenths: 0,
                        score_tenths: 0,
                    })
                    .collect(),
                unclaimed_riichi_sticks_awarded: 0,
                kan_points: Some(result.kan_points.to_vec()),
                point_deltas: Some(result.point_deltas.to_vec()),
            }),
            friend_match: value.friend_match,
            can_start_exit_vote: value.can_start_exit_vote,
            exit_vote: value.exit_vote.as_ref().map(|vote| ExitVoteResponse {
                initiator_seat: vote.initiator,
                remaining_ms: vote.deadline_ms.saturating_sub(now_ms),
                votes: vote.votes.clone(),
            }),
            terminated_by_exit_vote: value.terminated_by_exit_vote,
        }
    }
}

/// 一次杠带来的杠点增减。
#[derive(Serialize)]
struct KanPointsResponse {
    /// 单调递增的序号；客户端靠它认出「这是新的一次杠」而不是同一次的重发。
    id: u64,
    seat: u8,
    kind: &'static str,
    deltas: Vec<i32>,
}

#[derive(Serialize)]
struct ExitVoteResponse {
    initiator_seat: u8,
    remaining_ms: u64,
    votes: Vec<Option<bool>>,
}

#[derive(Serialize)]
struct HandSettlementResponse {
    reason: &'static str,
    tenpai_seats: Vec<u8>,
    point_deltas: Vec<i32>,
    points_before: Vec<i32>,
    points_after: Vec<i32>,
    winners: Vec<WinnerSettlementResponse>,
    /// 已经报告结算动画播完的座位。
    played_seats: Vec<u8>,
    /// 确认窗口剩下的时间；`null` 表示窗口还没开，确认按钮不该出现。
    confirm_remaining_ms: Option<u64>,
    confirmed_seats: Vec<u8>,
    from_seat: Option<u8>,
    ura_dora_indicators: Vec<TileResponse>,
    /// 触发的全交牌型。有值时役种表只写这一条，合计写「全交」。
    #[serde(skip_serializing_if = "Option::is_none")]
    all_in: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kan_point_deltas: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kan_points_after: Option<Vec<i32>>,
    /// 荒牌：本局不算，同一个庄家直接重开。
    #[serde(skip_serializing_if = "Option::is_none")]
    void_hand: Option<bool>,
}

#[derive(Serialize)]
struct WinnerSettlementResponse {
    seat: u8,
    han: u8,
    fu: u16,
    yakuman_multiplier: u8,
    limit: &'static str,
    points: u32,
    dealer: bool,
    yaku: Vec<YakuResponse>,
}

#[derive(Serialize)]
struct YakuResponse {
    name: &'static str,
    value: u32,
    yakuman: bool,
}

#[derive(Serialize)]
struct TurnActionsResponse {
    can_tsumo: bool,
    riichi_discard_tile_ids: Vec<u16>,
    riichi_discard_hints: Vec<DiscardWaitHintResponse>,
    tenpai_discard_hints: Vec<DiscardWaitHintResponse>,
    concealed_kan_tile_ids: Vec<[u16; 4]>,
    added_kan_options: Vec<AddedKanOptionResponse>,
    can_nine_terminals: bool,
    /// 冲击麻将的暗杠候选，给的是牌码；四张具体是哪四张由引擎挑。
    #[serde(skip_serializing_if = "Option::is_none")]
    impact_concealed_kan_tile_codes: Option<Vec<String>>,
    /// 可以加杠的副露编号。
    #[serde(skip_serializing_if = "Option::is_none")]
    impact_added_kan_meld_ids: Option<Vec<u16>>,
    /// 手持三张指示牌可以宣告的暗杠：只结算杠点，牌型仍是刻子。
    #[serde(skip_serializing_if = "Option::is_none")]
    impact_indicator_concealed_kan: Option<bool>,
}

#[derive(Serialize)]
struct DiscardWaitHintResponse {
    tile_id: u16,
    waiting_tiles: Vec<WaitingTileResponse>,
}

impl From<&DiscardWaitHint> for DiscardWaitHintResponse {
    fn from(value: &DiscardWaitHint) -> Self {
        Self {
            tile_id: value.tile_id(),
            waiting_tiles: value
                .waiting_tiles()
                .iter()
                .map(|tile| WaitingTileResponse {
                    code: tile.code().to_owned(),
                    has_yaku: tile.has_yaku(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct AddedKanOptionResponse {
    meld_id: u8,
    tile_id: u16,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReactionResponse {
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
    /// 冲击麻将的碰。手上凑数的两张由引擎自己挑，所以不带牌号。
    ///
    /// `indicator` 为真时被碰的是财神指示牌：杠点按明杠结算，牌型仍是刻子。
    ImpactPon {
        indicator: bool,
    },
    ImpactOpenKan,
}

impl ReactionResponse {
    fn impact(value: &mamahjong_application::ImpactReactionOptionsView) -> Vec<Self> {
        let mut options = Vec::new();
        if value.can_pon {
            options.push(Self::ImpactPon {
                indicator: value.pon_is_indicator,
            });
        }
        if value.can_open_kan {
            options.push(Self::ImpactOpenKan);
        }
        options
    }
}

impl From<&Reaction> for ReactionResponse {
    fn from(value: &Reaction) -> Self {
        match value {
            Reaction::Ron => Self::Ron,
            Reaction::Chi { hand_tiles } => Self::Chi {
                tile_ids: hand_tiles.map(mahjong_riichi::TileId::value),
            },
            Reaction::Pon { hand_tiles } => Self::Pon {
                tile_ids: hand_tiles.map(mahjong_riichi::TileId::value),
            },
            Reaction::OpenKan { hand_tiles } => Self::OpenKan {
                tile_ids: hand_tiles.map(mahjong_riichi::TileId::value),
            },
            Reaction::Pass => unreachable!("pass is not an available reaction"),
        }
    }
}

#[derive(Serialize)]
struct ProgressResponse {
    round_wind: &'static str,
    round_number: u8,
    dealer: u8,
    honba: u32,
    riichi_sticks: u32,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PhaseResponse {
    AwaitingTurnAction { seat: u8 },
    AwaitingDiscard { seat: u8 },
    AwaitingResponses { trigger_seat: u8 },
    AwaitingKanAnimation { seat: u8 },
    Ended { reason: &'static str },
}

impl PhaseResponse {
    /// 冲击麻将的阶段。引擎那边已经把它压成了三元组，这里只是换个形状。
    fn impact(kind: &'static str, seat: Option<u8>, reason: Option<&'static str>) -> Self {
        match kind {
            "awaiting_discard" => Self::AwaitingDiscard {
                seat: seat.unwrap_or_default(),
            },
            "awaiting_responses" => Self::AwaitingResponses {
                trigger_seat: seat.unwrap_or_default(),
            },
            "awaiting_kan_animation" => Self::AwaitingKanAnimation {
                seat: seat.unwrap_or_default(),
            },
            "ended" => Self::Ended {
                reason: reason.unwrap_or("exhaustive_draw"),
            },
            _ => Self::AwaitingTurnAction {
                seat: seat.unwrap_or_default(),
            },
        }
    }
}

impl From<HandPhase> for PhaseResponse {
    fn from(value: HandPhase) -> Self {
        match value {
            HandPhase::AwaitingTurnAction { seat } => {
                Self::AwaitingTurnAction { seat: seat.index() }
            }
            HandPhase::AwaitingDiscard { seat } => Self::AwaitingDiscard { seat: seat.index() },
            HandPhase::AwaitingResponses { trigger_seat } => Self::AwaitingResponses {
                trigger_seat: trigger_seat.index(),
            },
            HandPhase::Ended { reason } => Self::Ended {
                reason: end_reason_name(reason),
            },
        }
    }
}

#[derive(Serialize)]
struct MatchPlayerResponse {
    user_id: String,
    seat: u8,
    nickname: String,
    avatar_path: Option<String>,
    /// 这一家用的角色，客户端照它挑操作语音。
    character_id: Option<String>,
    character_illustration_path: Option<String>,
    points: i32,
    concealed_tiles: Option<Vec<TileResponse>>,
    concealed_tile_count: usize,
    drawn_tile_id: Option<u16>,
    melds: Vec<MeldResponse>,
    discards: Vec<DiscardResponse>,
    riichi_status: &'static str,
    waiting_tiles: Vec<WaitingTileResponse>,
    furiten: bool,
    /// 冲击麻将的杠点账，和点数分开记，可以为负、不设下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    kan_points: Option<i32>,
    /// 本局已经开出的真杠数。三杠触发全交，指示牌碰/暗杠不计。
    #[serde(skip_serializing_if = "Option::is_none")]
    kan_count: Option<u8>,
    /// 连续打出字牌或财神的次数。数到 11 触发连打十一风全交。
    #[serde(skip_serializing_if = "Option::is_none")]
    honor_streak: Option<u32>,
    /// 立直音乐路径。玩家选了立直曲目且有对应文件就是它的路径，否则为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    riichi_music_path: Option<String>,
}

#[derive(Serialize)]
struct WaitingTileResponse {
    code: String,
    has_yaku: bool,
}

impl MatchPlayerResponse {
    fn new(value: &ObserverPlayer, application: &Application) -> Self {
        let portrait = portrait(application, value.player().user_id());
        Self {
            user_id: value.player().user_id().as_str().to_owned(),
            seat: value.player().seat().index(),
            nickname: value.player().nickname().to_owned(),
            avatar_path: portrait.avatar_path,
            character_id: portrait.character_id,
            character_illustration_path: portrait.character_illustration_path,
            points: value.points(),
            concealed_tiles: value
                .concealed_tiles()
                .map(|tiles| tiles.iter().copied().map(TileResponse::from).collect()),
            concealed_tile_count: value.concealed_tile_count(),
            drawn_tile_id: value.drawn_tile_id().map(mahjong_riichi::TileId::value),
            melds: value.melds().iter().map(MeldResponse::from).collect(),
            discards: value.discards().iter().map(DiscardResponse::from).collect(),
            riichi_status: match value.riichi_status() {
                RiichiStatus::None => "none",
                RiichiStatus::Pending => "pending",
                RiichiStatus::Established => "established",
            },
            waiting_tiles: value
                .waiting_tiles()
                .iter()
                .map(|tile| WaitingTileResponse {
                    code: tile.code().to_owned(),
                    has_yaku: tile.has_yaku(),
                })
                .collect(),
            furiten: value.is_furiten(),
            kan_points: None,
            kan_count: None,
            honor_streak: None,
            riichi_music_path: portrait.riichi_music_path,
        }
    }

    /// 冲击麻将的一位玩家。立直状态、听牌提示、振听在这套规则里都不存在。
    fn impact(value: &ObserverImpactPlayer, application: &Application) -> Self {
        let portrait = portrait(application, value.player.user_id());
        Self {
            user_id: value.player.user_id().as_str().to_owned(),
            seat: value.player.seat(),
            nickname: value.player.nickname().to_owned(),
            avatar_path: portrait.avatar_path,
            character_id: portrait.character_id,
            character_illustration_path: portrait.character_illustration_path,
            points: value.points,
            concealed_tiles: value
                .concealed_tiles
                .as_ref()
                .map(|tiles| tiles.iter().map(TileResponse::from).collect()),
            concealed_tile_count: value.concealed_tile_count,
            drawn_tile_id: value.drawn_tile_id,
            melds: value.melds.iter().map(MeldResponse::from).collect(),
            discards: value.discards.iter().map(DiscardResponse::from).collect(),
            riichi_status: "none",
            waiting_tiles: Vec::new(),
            furiten: false,
            kan_points: Some(value.kan_points),
            kan_count: Some(value.kan_count),
            honor_streak: Some(value.honor_streak),
            riichi_music_path: portrait.riichi_music_path,
        }
    }
}

/// 一位玩家的头像与立绘路径。
///
/// 和 `MatchPlayerResponse::new` 里那段挑角色的逻辑同义：选中的角色停用或查不到
/// 就回落到默认角色，衣装按用户当前选的那套取。
fn portrait(application: &Application, user_id: &mahjong_core::UserId) -> Portrait {
    let user = application.user(user_id).ok();
    let selected_character_id = user
        .as_ref()
        .and_then(|user| user.profile().selected_character())
        .map(|character| character.id());
    let character = application.list_characters().ok().and_then(|characters| {
        characters
            .into_iter()
            .find(|character| {
                character.enabled()
                    && selected_character_id.is_some_and(|selected| selected == character.id())
            })
            .or_else(|| application.default_character().ok())
    });
    let riichi_music_path = user
        .as_ref()
        .and_then(|user| user.profile().selected_riichi_music_id())
        .and_then(|track_id| {
            application
                .list_music_tracks()
                .ok()?
                .into_iter()
                .find(|track| track.id() == track_id && track.enabled())
                .map(|track| track.audio_path().to_owned())
        });
    Portrait {
        avatar_path: user
            .as_ref()
            .and_then(|user| user.profile().avatar_path().map(str::to_owned)),
        character_id: character
            .as_ref()
            .map(|character| character.id().to_owned()),
        character_illustration_path: character.as_ref().map(|character| {
            user.as_ref().map_or_else(
                || character.illustration_path().to_owned(),
                |user| selected_illustration_path(user, character),
            )
        }),
        riichi_music_path,
    }
}

/// 一位玩家摆在牌桌上的那套形象：头像、角色、当前衣装的立绘。
struct Portrait {
    avatar_path: Option<String>,
    /// 客户端照它挑这一家该喊哪个角色的语音。
    character_id: Option<String>,
    character_illustration_path: Option<String>,
    /// 玩家选的立直曲目的文件路径；没选就为空。
    riichi_music_path: Option<String>,
}

fn selected_illustration_path(user: &User, character: &Character) -> String {
    user.profile()
        .selected_outfit_id()
        .and_then(|selected_outfit_id| {
            character
                .outfits()
                .iter()
                .find(|outfit| outfit.id == selected_outfit_id)
        })
        .map_or_else(
            || character.illustration_path().to_owned(),
            |outfit| outfit.illustration_path.clone(),
        )
}

#[derive(Serialize)]
struct TileResponse {
    id: u16,
    code: String,
}

impl From<Tile> for TileResponse {
    fn from(value: Tile) -> Self {
        Self {
            id: value.id().value(),
            code: value.to_string(),
        }
    }
}

impl From<&ImpactTileView> for TileResponse {
    fn from(value: &ImpactTileView) -> Self {
        Self {
            id: value.id,
            code: value.code.clone(),
        }
    }
}

#[derive(Serialize)]
struct MeldResponse {
    id: u16,
    kind: &'static str,
    tiles: Vec<TileResponse>,
    called_from: Option<u8>,
    called_tile_id: Option<u16>,
}

impl From<&Meld> for MeldResponse {
    fn from(value: &Meld) -> Self {
        Self {
            id: u16::from(value.id().value()),
            kind: match value.kind() {
                MeldKind::Chi => "chi",
                MeldKind::Pon => "pon",
                MeldKind::OpenKan => "open_kan",
                MeldKind::ConcealedKan => "concealed_kan",
                MeldKind::AddedKan => "added_kan",
            },
            tiles: value
                .tiles()
                .iter()
                .copied()
                .map(TileResponse::from)
                .collect(),
            called_from: value.called_from().map(mahjong_riichi::Seat::index),
            called_tile_id: value.called_tile().map(mahjong_riichi::TileId::value),
        }
    }
}

impl From<&ImpactMeldView> for MeldResponse {
    fn from(value: &ImpactMeldView) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            tiles: value.tiles.iter().map(TileResponse::from).collect(),
            called_from: value.called_from,
            called_tile_id: value.called_tile_id,
        }
    }
}

#[derive(Serialize)]
struct DiscardResponse {
    tile: TileResponse,
    tsumogiri: bool,
    riichi_declared: bool,
    claimed_by: Option<u8>,
    /// 这张牌被鸣走了。立直那边靠 `claimed_by` 就能判，冲击麻将只记了有没有。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    claimed: bool,
}

impl From<&Discard> for DiscardResponse {
    fn from(value: &Discard) -> Self {
        Self {
            tile: TileResponse::from(value.tile()),
            tsumogiri: value.is_tsumogiri(),
            riichi_declared: value.is_riichi_declaration(),
            claimed_by: value.claimed_by().map(mahjong_riichi::Seat::index),
            claimed: value.claimed_by().is_some(),
        }
    }
}

impl From<&ImpactDiscardView> for DiscardResponse {
    fn from(value: &ImpactDiscardView) -> Self {
        Self {
            tile: TileResponse::from(&value.tile),
            // 冲击麻将的牌河不记摸切，也没有立直宣言牌。
            tsumogiri: false,
            riichi_declared: false,
            // 谁鸣走的没记，只记这张被不被鸣了：牌河照样要画上斜杠。
            claimed_by: None,
            claimed: value.called,
        }
    }
}

#[derive(Serialize)]
struct MatchResultResponse {
    end_reason: &'static str,
    hand_count: u32,
    final_points: Vec<i32>,
    placements: Vec<PlacementResponse>,
    unclaimed_riichi_sticks_awarded: u32,
    /// 冲击麻将整场的杠点结余，结算页要和点数增减一起列。
    #[serde(skip_serializing_if = "Option::is_none")]
    kan_points: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    point_deltas: Option<Vec<i32>>,
}

impl From<&MatchResult> for MatchResultResponse {
    fn from(value: &MatchResult) -> Self {
        Self {
            end_reason: match value.end_reason() {
                mahjong_riichi::MatchEndReason::ScheduledEnd => "scheduled_end",
                mahjong_riichi::MatchEndReason::Tobi => "tobi",
                mahjong_riichi::MatchEndReason::AgariYame => "agari_yame",
            },
            hand_count: value.hand_count(),
            final_points: value.final_points().to_vec(),
            placements: value
                .placements()
                .iter()
                .map(|placement| PlacementResponse {
                    seat: placement.seat().index(),
                    rank: placement.rank(),
                    points: placement.points(),
                    uma_tenths: placement.uma_tenths(),
                    oka_tenths: placement.oka_tenths(),
                    score_tenths: placement.score_tenths(),
                })
                .collect(),
            unclaimed_riichi_sticks_awarded: value.unclaimed_riichi_sticks_awarded(),
            kan_points: None,
            point_deltas: None,
        }
    }
}

#[derive(Serialize)]
struct PlacementResponse {
    seat: u8,
    rank: u8,
    points: i32,
    uma_tenths: i32,
    oka_tenths: i32,
    score_tenths: i32,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::PhaseResponse;

    #[test]
    fn impact_kan_animation_remains_a_distinct_client_phase() {
        let phase = PhaseResponse::impact("awaiting_kan_animation", Some(2), None);
        assert_eq!(
            serde_json::to_value(phase).expect("serialize phase"),
            json!({"kind": "awaiting_kan_animation", "seat": 2}),
        );
    }
}
