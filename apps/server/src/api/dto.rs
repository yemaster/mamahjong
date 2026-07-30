use mamahjong_application::{
    AccountStatus, CharacterSummary, GameRuleSnapshot, RankSummary, Room, RoomLifecycle,
    RoomVisibility, Session, TitleSummary, User, UserProfile,
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
