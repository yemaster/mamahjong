use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use mahjong_core::TicketId;
use mahjong_riichi::RiichiVariant;
use mamahjong_application::{MatchmakingStatus, MatchmakingTicket};
use serde::{Deserialize, Serialize};

use super::auth::AuthenticatedUser;
use super::error::ApiError;
use crate::{AppState, AuditDraft};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/matchmaking-tickets", post(enter))
        .route(
            "/matchmaking-tickets/{ticket_id}",
            get(get_ticket).delete(cancel),
        )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnterRequest {
    rule_set_id: String,
}

async fn enter(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    payload: Result<Json<EnterRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<TicketResponse>), ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let variant = parse_variant(&payload.rule_set_id)?;
    let ticket =
        state
            .application()
            .enter_matchmaking(user.user().id(), variant, state.now_ms())?;
    if let MatchmakingStatus::Matched { match_id, .. } = ticket.status() {
        state
            .persist_match(user.user().id(), match_id)
            .await
            .map_err(|error| {
                tracing::error!(%error, %match_id, "failed to persist matched game");
                ApiError::internal()
            })?;
    }
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "matchmaking",
            action: match ticket.status() {
                MatchmakingStatus::Matched { .. } => "matchmaking.matched",
                _ => "matchmaking.entered",
            },
            actor_id: Some(user.user().id().as_str().to_owned()),
            target_type: "ticket",
            target_id: Some(ticket.id().as_str().to_owned()),
            outcome: "success",
            detail: match ticket.status() {
                MatchmakingStatus::Matched { .. } => "匹配完成",
                _ => "已进入匹配",
            }
            .to_owned(),
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to record matchmaking audit");
            ApiError::internal()
        })?;
    Ok((StatusCode::CREATED, Json(TicketResponse::from(&ticket))))
}

async fn get_ticket(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(ticket_id): Path<String>,
) -> Result<Json<TicketResponse>, ApiError> {
    let ticket_id = parse_ticket_id(ticket_id)?;
    let ticket = state
        .application()
        .matchmaking_ticket(user.user().id(), &ticket_id)?;
    Ok(Json(TicketResponse::from(&ticket)))
}

async fn cancel(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(ticket_id): Path<String>,
) -> Result<Json<TicketResponse>, ApiError> {
    let ticket_id = parse_ticket_id(ticket_id)?;
    let ticket = state
        .application()
        .cancel_matchmaking(user.user().id(), &ticket_id)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "matchmaking",
            action: "matchmaking.cancelled",
            actor_id: Some(user.user().id().as_str().to_owned()),
            target_type: "ticket",
            target_id: Some(ticket_id.as_str().to_owned()),
            outcome: "success",
            detail: "已取消匹配".to_owned(),
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to record matchmaking cancellation audit");
            ApiError::internal()
        })?;
    Ok(Json(TicketResponse::from(&ticket)))
}

/// 匹配队列只收立直麻将。
///
/// 冲击麻将只开好友房，`impact/*` 在这里就被挡回去，不会走到应用层。
fn parse_variant(value: &str) -> Result<RiichiVariant, ApiError> {
    match value {
        "riichi/yonma" => Ok(RiichiVariant::Yonma),
        "riichi/sanma" => Ok(RiichiVariant::Sanma),
        _ => Err(ApiError::invalid_rule_set()),
    }
}

fn parse_ticket_id(value: String) -> Result<TicketId, ApiError> {
    TicketId::parse(value).map_err(|_| ApiError::invalid_id())
}

#[derive(Serialize)]
struct TicketResponse {
    schema: &'static str,
    id: String,
    rule_set_id: &'static str,
    status: &'static str,
    room_id: Option<String>,
    match_id: Option<String>,
}

impl From<&MatchmakingTicket> for TicketResponse {
    fn from(value: &MatchmakingTicket) -> Self {
        let (status, room_id, match_id) = match value.status() {
            MatchmakingStatus::Waiting => ("waiting", None, None),
            MatchmakingStatus::Matched { room_id, match_id } => (
                "matched",
                Some(room_id.as_str().to_owned()),
                Some(match_id.as_str().to_owned()),
            ),
            MatchmakingStatus::Cancelled => ("cancelled", None, None),
        };
        Self {
            schema: "matchmaking_ticket.v1",
            id: value.id().as_str().to_owned(),
            rule_set_id: value.variant().rule_set_id(),
            status,
            room_id,
            match_id,
        }
    }
}
