use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use mahjong_core::RoomId;
use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
use mamahjong_application::{CreateRoom, RoomRuleSelection, RoomVisibility, UpdateRoom};
use serde::Deserialize;

use super::auth::AuthenticatedUser;
use super::dto::{RoomListResponse, RoomResponse, StartRoomResponse};
use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/rooms", get(list_rooms).post(create_room))
        .route("/rooms/{room_id}", get(get_room).patch(update_room))
        .route(
            "/rooms/{room_id}/members",
            post(join_room).delete(leave_room),
        )
        .route("/rooms/{room_id}/members/me/readiness", put(set_ready))
        .route("/rooms/{room_id}/matches", post(start_room))
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VisibilityRequest {
    Public,
    Private,
}

impl From<VisibilityRequest> for RoomVisibility {
    fn from(value: VisibilityRequest) -> Self {
        match value {
            VisibilityRequest::Public => Self::Public,
            VisibilityRequest::Private => Self::Private,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleSelectionRequest {
    rule_set_id: String,
    #[serde(default)]
    config: RoomRuleRequest,
}

impl RuleSelectionRequest {
    fn into_application(self) -> Result<RoomRuleSelection, ApiError> {
        let variant = match self.rule_set_id.as_str() {
            "riichi/yonma" => RiichiVariant::Yonma,
            "riichi/sanma" => RiichiVariant::Sanma,
            _ => {
                return Err(ApiError::from(
                    mamahjong_application::ApplicationError::new(
                        mamahjong_application::ErrorCode::InvalidRuleConfiguration,
                        "unsupported rule_set_id",
                    ),
                ));
            }
        };
        Ok(RoomRuleSelection::Riichi {
            variant,
            request: self.config,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRoomRequest {
    name: String,
    visibility: VisibilityRequest,
    rules: RuleSelectionRequest,
}

async fn create_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    payload: Result<Json<CreateRoomRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<RoomResponse>), ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let room = state.application().create_room(
        user.user().id(),
        CreateRoom {
            name: payload.name,
            visibility: payload.visibility.into(),
            rules: payload.rules.into_application()?,
        },
    )?;
    Ok((StatusCode::CREATED, Json(RoomResponse::try_from(&room)?)))
}

async fn list_rooms(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<RoomListResponse>, ApiError> {
    let rooms = state.application().list_rooms()?;
    Ok(Json(RoomListResponse::new(&rooms)?))
}

async fn get_room(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(room_id): Path<String>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let room = state.application().room(&room_id)?;
    Ok(Json(RoomResponse::try_from(&room)?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionRequest {
    expected_version: u64,
}

async fn join_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let room =
        state
            .application()
            .join_room(user.user().id(), &room_id, payload.expected_version)?;
    Ok(Json(RoomResponse::try_from(&room)?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadinessRequest {
    expected_version: u64,
    ready: bool,
}

async fn set_ready(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<ReadinessRequest>, JsonRejection>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let room = state.application().set_ready(
        user.user().id(),
        &room_id,
        payload.expected_version,
        payload.ready,
    )?;
    Ok(Json(RoomResponse::try_from(&room)?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRoomRequest {
    expected_version: u64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    visibility: Option<VisibilityRequest>,
    #[serde(default)]
    rules: Option<RuleSelectionRequest>,
}

async fn update_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<UpdateRoomRequest>, JsonRejection>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let room = state.application().update_room(
        user.user().id(),
        &room_id,
        UpdateRoom {
            expected_version: payload.expected_version,
            name: payload.name,
            visibility: payload.visibility.map(Into::into),
            rules: payload
                .rules
                .map(RuleSelectionRequest::into_application)
                .transpose()?,
        },
    )?;
    Ok(Json(RoomResponse::try_from(&room)?))
}

async fn leave_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let room =
        state
            .application()
            .leave_room(user.user().id(), &room_id, payload.expected_version)?;
    Ok(Json(RoomResponse::try_from(&room)?))
}

async fn start_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<StartRoomResponse>), ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let (room, match_id) =
        state
            .application()
            .start_room(user.user().id(), &room_id, payload.expected_version)?;
    state
        .persist_match(user.user().id(), &match_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %match_id, "failed to persist initial match record");
            ApiError::internal()
        })?;
    Ok((
        StatusCode::CREATED,
        Json(StartRoomResponse {
            schema: "match_started.v1",
            match_id: match_id.as_str().to_owned(),
            room: RoomResponse::try_from(&room)?,
        }),
    ))
}

fn parse_room_id(value: String) -> Result<RoomId, ApiError> {
    RoomId::parse(value).map_err(|_| ApiError::invalid_id())
}
