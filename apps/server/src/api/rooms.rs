use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use mahjong_core::{RoomId, UserId};
use mahjong_impact::ImpactRoomRuleRequest;
use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
use mamahjong_application::{CreateRoom, RoomRuleSelection, RoomVisibility, UpdateRoom};
use serde::Deserialize;

use super::auth::AuthenticatedUser;
use super::dto::{RoomListResponse, RoomResponse, StartRoomResponse};
use super::error::ApiError;
use crate::{AppState, AuditDraft};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/rooms", get(list_rooms).post(create_room))
        .route("/rooms/{room_id}", get(get_room).patch(update_room))
        .route(
            "/rooms/{room_id}/members",
            post(join_room).delete(leave_room),
        )
        .route(
            "/rooms/{room_id}/members/me",
            axum::routing::delete(leave_room_on_exit),
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

/// 建房/改房时选的规则。
///
/// `config` 的形状由 `rule_set_id` 决定，两套规则的字段完全不同，所以先收成
/// `serde_json::Value`，认出家族之后再按各自的 `deny_unknown_fields` 严格解一遍。
/// 拿错家族的配置去建房，会在这一步就被挡下来，而不是带着一半默认值开局。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleSelectionRequest {
    rule_set_id: String,
    #[serde(default)]
    config: serde_json::Value,
}

impl RuleSelectionRequest {
    fn into_application(self) -> Result<RoomRuleSelection, ApiError> {
        match self.rule_set_id.as_str() {
            "riichi/yonma" => Ok(RoomRuleSelection::Riichi {
                variant: RiichiVariant::Yonma,
                request: parse_config::<RoomRuleRequest>(self.config)?,
            }),
            "riichi/sanma" => Ok(RoomRuleSelection::Riichi {
                variant: RiichiVariant::Sanma,
                request: parse_config::<RoomRuleRequest>(self.config)?,
            }),
            "impact/yonma" => Ok(RoomRuleSelection::Impact {
                request: parse_config::<ImpactRoomRuleRequest>(self.config)?,
            }),
            _ => Err(rule_configuration_error("unsupported rule_set_id")),
        }
    }
}

/// 缺省的 `config` 就是「全用默认值」，其余一律严格解析。
fn parse_config<T>(value: serde_json::Value) -> Result<T, ApiError>
where
    T: Default + serde::de::DeserializeOwned,
{
    if value.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(value)
        .map_err(|error| rule_configuration_error(format!("invalid rule config: {error}")))
}

fn rule_configuration_error(message: impl Into<String>) -> ApiError {
    ApiError::from(mamahjong_application::ApplicationError::new(
        mamahjong_application::ErrorCode::InvalidRuleConfiguration,
        message,
    ))
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
    record_room_audit(
        &state,
        user.user().id(),
        "room.created",
        room.id().as_str(),
        "房间已创建",
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(RoomResponse::new(&room, state.application())?),
    ))
}

async fn list_rooms(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<RoomListResponse>, ApiError> {
    let rooms = state.application().list_rooms()?;
    Ok(Json(RoomListResponse::new(&rooms, state.application())?))
}

async fn get_room(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(room_id): Path<String>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let room = state.application().room(&room_id)?;
    Ok(Json(RoomResponse::new(&room, state.application())?))
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
    record_room_audit(
        &state,
        user.user().id(),
        "room.joined",
        room_id.as_str(),
        "加入房间",
    )
    .await?;
    Ok(Json(RoomResponse::new(&room, state.application())?))
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
    Ok(Json(RoomResponse::new(&room, state.application())?))
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
    record_room_audit(
        &state,
        user.user().id(),
        "room.updated",
        room_id.as_str(),
        "房间设置已更新",
    )
    .await?;
    Ok(Json(RoomResponse::new(&room, state.application())?))
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
    record_room_audit(
        &state,
        user.user().id(),
        "room.left",
        room_id.as_str(),
        "离开房间",
    )
    .await?;
    Ok(Json(RoomResponse::new(&room, state.application())?))
}

async fn leave_room_on_exit(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room_id = parse_room_id(room_id)?;
    let room = state
        .application()
        .leave_room_current(user.user().id(), &room_id)?;
    record_room_audit(
        &state,
        user.user().id(),
        "room.left",
        room_id.as_str(),
        "离开房间",
    )
    .await?;
    Ok(Json(RoomResponse::new(&room, state.application())?))
}

async fn start_room(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(room_id): Path<String>,
    payload: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<StartRoomResponse>), ApiError> {
    let room_id = parse_room_id(room_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let (room, match_id) = state.application().start_room(
        user.user().id(),
        &room_id,
        payload.expected_version,
        state.now_ms(),
    )?;
    state
        .persist_match(user.user().id(), &match_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %match_id, "failed to persist initial match record");
            ApiError::internal()
        })?;
    record_room_audit(
        &state,
        user.user().id(),
        "game.started",
        match_id.as_str(),
        "对局已开始",
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(StartRoomResponse {
            schema: "match_started.v1",
            match_id: match_id.as_str().to_owned(),
            room: RoomResponse::new(&room, state.application())?,
        }),
    ))
}

fn parse_room_id(value: String) -> Result<RoomId, ApiError> {
    RoomId::parse(value).map_err(|_| ApiError::invalid_id())
}

async fn record_room_audit(
    state: &AppState,
    actor: &UserId,
    action: &'static str,
    target_id: &str,
    detail: &'static str,
) -> Result<(), ApiError> {
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: if action.starts_with("game.") {
                "game"
            } else {
                "room"
            },
            action,
            actor_id: Some(actor.as_str().to_owned()),
            target_type: if action.starts_with("game.") {
                "match"
            } else {
                "room"
            },
            target_id: Some(target_id.to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        })
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::error!(%error, action, target_id, "failed to record room audit");
            ApiError::internal()
        })
}
