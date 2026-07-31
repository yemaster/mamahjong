use mahjong_riichi::{
    Discard, EndReason, HandPhase, MatchResult, Meld, MeldKind, Reaction, RiichiStatus, Tile, Wind,
};
use mamahjong_application::{
    AccountStatus, CharacterSummary, GameRuleSnapshot, ObserverMatch, ObserverPlayer, RankSummary,
    Room, RoomLifecycle, RoomVisibility, Session, TitleSummary, User, UserProfile,
};
use serde::Serialize;

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
            profile: ProfileResponse::from(user.profile()),
        }
    }
}

#[derive(Serialize)]
pub(super) struct ProfileResponse {
    nickname: String,
    equipped_title: Option<TitleResponse>,
    selected_character: Option<CharacterResponse>,
    ranks: Vec<RankResponse>,
}

impl From<&UserProfile> for ProfileResponse {
    fn from(profile: &UserProfile) -> Self {
        Self {
            nickname: profile.nickname().as_str().to_owned(),
            equipped_title: profile.equipped_title().map(TitleResponse::from),
            selected_character: profile.selected_character().map(CharacterResponse::from),
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
    pub(super) fn new(rooms: &[Room]) -> Result<Self, ApiError> {
        Ok(Self {
            schema: "room_list.v1",
            rooms: rooms
                .iter()
                .map(RoomResponse::try_from)
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
    rule_snapshot: serde_json::Value,
    members: Vec<RoomMemberResponse>,
    active_match_id: Option<String>,
}

impl TryFrom<&Room> for RoomResponse {
    type Error = ApiError;

    fn try_from(room: &Room) -> Result<Self, Self::Error> {
        let rule_snapshot = match room.rule_snapshot() {
            GameRuleSnapshot::Riichi(snapshot) => {
                serde_json::to_value(snapshot).map_err(|_| ApiError::internal())?
            }
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
            rule_snapshot,
            members: room
                .members()
                .iter()
                .map(|member| RoomMemberResponse {
                    user_id: member.user_id().as_str().to_owned(),
                    seat: member.seat(),
                    nickname: member.nickname().to_owned(),
                    ready: member.ready(),
                })
                .collect(),
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
}

#[derive(Serialize)]
pub(super) struct StartRoomResponse {
    pub(super) schema: &'static str,
    pub(super) match_id: String,
    pub(super) room: RoomResponse,
}

#[derive(Serialize)]
pub(super) struct MatchViewResponse {
    schema: &'static str,
    id: String,
    version: u64,
    event_sequence: u64,
    hand_index: u32,
    observer_seat: u8,
    progress: ProgressResponse,
    phase: PhaseResponse,
    remaining_live_draws: usize,
    dora_indicators: Vec<TileResponse>,
    players: Vec<MatchPlayerResponse>,
    available_reactions: Vec<ReactionResponse>,
    result: Option<MatchResultResponse>,
}

impl From<&ObserverMatch> for MatchViewResponse {
    fn from(value: &ObserverMatch) -> Self {
        let progress = value.progress();
        Self {
            schema: "match_view.v1",
            id: value.id().as_str().to_owned(),
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
                .map(MatchPlayerResponse::from)
                .collect(),
            available_reactions: value
                .available_reactions()
                .iter()
                .map(ReactionResponse::from)
                .collect(),
            result: value.result().map(MatchResultResponse::from),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReactionResponse {
    Ron,
    Chi { tile_ids: [u16; 2] },
    Pon { tile_ids: [u16; 2] },
    OpenKan { tile_ids: [u16; 3] },
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
    Ended { reason: &'static str },
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
    points: i32,
    concealed_tiles: Option<Vec<TileResponse>>,
    concealed_tile_count: usize,
    drawn_tile_id: Option<u16>,
    melds: Vec<MeldResponse>,
    discards: Vec<DiscardResponse>,
    riichi_status: &'static str,
}

impl From<&ObserverPlayer> for MatchPlayerResponse {
    fn from(value: &ObserverPlayer) -> Self {
        Self {
            user_id: value.player().user_id().as_str().to_owned(),
            seat: value.player().seat().index(),
            nickname: value.player().nickname().to_owned(),
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
        }
    }
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

#[derive(Serialize)]
struct MeldResponse {
    id: u8,
    kind: &'static str,
    tiles: Vec<TileResponse>,
    called_from: Option<u8>,
    called_tile_id: Option<u16>,
}

impl From<&Meld> for MeldResponse {
    fn from(value: &Meld) -> Self {
        Self {
            id: value.id().value(),
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

#[derive(Serialize)]
struct DiscardResponse {
    tile: TileResponse,
    tsumogiri: bool,
    riichi_declared: bool,
    claimed_by: Option<u8>,
}

impl From<&Discard> for DiscardResponse {
    fn from(value: &Discard) -> Self {
        Self {
            tile: TileResponse::from(value.tile()),
            tsumogiri: value.is_tsumogiri(),
            riichi_declared: value.is_riichi_declaration(),
            claimed_by: value.claimed_by().map(mahjong_riichi::Seat::index),
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

const fn wind_name(value: Wind) -> &'static str {
    match value {
        Wind::East => "east",
        Wind::South => "south",
        Wind::West => "west",
        Wind::North => "north",
    }
}

const fn end_reason_name(value: EndReason) -> &'static str {
    match value {
        EndReason::ExhaustiveDraw => "exhaustive_draw",
        EndReason::NineTerminals => "nine_terminals",
        EndReason::FourWinds => "four_winds",
        EndReason::FourKans => "four_kans",
        EndReason::FourRiichi => "four_riichi",
        EndReason::Tsumo => "tsumo",
        EndReason::Ron => "ron",
    }
}
