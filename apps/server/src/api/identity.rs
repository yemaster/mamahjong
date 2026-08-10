use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post, put};
use mahjong_core::UserId;
use mamahjong_application::{RegisterUser, UpdatePresentation, UpdateProfile};
use serde::{Deserialize, Serialize};

use super::auth::AuthenticatedUser;
use super::dto::{AuthResponse, UserResponse};
use super::error::ApiError;
use crate::{AppState, AuditDraft};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/registrations", post(register))
        .route("/sessions", post(login))
        .route("/sessions/me/revoke-others", post(revoke_other_sessions))
        .route("/users/me", get(me))
        .route("/users/me/activity", get(activity))
        .route("/users/me/profile", patch(update_profile))
        .route("/users/me/presentation", put(update_presentation))
        .route("/users/{user_id}/profile", get(profile))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationRequest {
    login_name: String,
    password: String,
    nickname: String,
}

async fn register(
    State(state): State<AppState>,
    payload: Result<Json<RegistrationRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let application = state.application().clone();
    let command = RegisterUser {
        login_name: payload.login_name,
        password: payload.password,
        nickname: payload.nickname,
    };
    let (user, session) = tokio::task::spawn_blocking(move || application.register(command))
        .await
        .map_err(|_| ApiError::internal())??;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "auth",
            action: "user.registration.succeeded",
            actor_id: Some(user.id().as_str().to_owned()),
            target_type: "user",
            target_id: Some(user.id().as_str().to_owned()),
            outcome: "success",
            detail: "账号注册成功".to_owned(),
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to record registration audit");
            ApiError::internal()
        })?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse::new(&user, &session)),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    login_name: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Json<AuthResponse>, ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let application = state.application().clone();
    let result = tokio::task::spawn_blocking(move || {
        application.login(&payload.login_name, &payload.password)
    })
    .await
    .map_err(|_| ApiError::internal())?;
    let (user, session) = match result {
        Ok(authenticated) => authenticated,
        Err(error) => {
            let _ = state
                .record_audit(AuditDraft {
                    severity: "warn",
                    category: "auth",
                    action: "user.login.failed",
                    actor_id: None,
                    target_type: "session",
                    target_id: None,
                    outcome: "failure",
                    detail: "登录失败".to_owned(),
                })
                .await;
            return Err(ApiError::from(error));
        }
    };
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "auth",
            action: "user.login.succeeded",
            actor_id: Some(user.id().as_str().to_owned()),
            target_type: "session",
            target_id: None,
            outcome: "success",
            detail: "登录成功".to_owned(),
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to record login audit");
            ApiError::internal()
        })?;
    Ok(Json(AuthResponse::new(&user, &session)))
}

async fn me(user: AuthenticatedUser) -> Json<UserResponse> {
    Json(UserResponse::from(user.user()))
}

async fn revoke_other_sessions(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    req: axum::extract::Request,
) -> Result<StatusCode, ApiError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(ApiError::missing_bearer)?
        .to_owned();
    let user_id = user.user().id().clone();
    let application = state.application().clone();
    let revoke_user_id = user_id.clone();
    tokio::task::spawn_blocking(move || application.revoke_other_sessions(&user_id, &token))
        .await
        .map_err(|_| ApiError::internal())??;
    state.realtime().revoke_user_connections(&revoke_user_id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct UserActivityResponse {
    schema: &'static str,
    kind: &'static str,
    room_id: Option<String>,
    match_id: Option<String>,
    ticket_id: Option<String>,
}

async fn activity(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<UserActivityResponse>, ApiError> {
    if let Some(room) = state.application().current_room(user.user().id())? {
        let active_match_id = room
            .active_match_id()
            .map(|match_id| match_id.as_str().to_owned());
        return Ok(Json(UserActivityResponse {
            schema: "user_activity.v1",
            kind: if active_match_id.is_some() {
                "game"
            } else {
                "room"
            },
            room_id: Some(room.id().as_str().to_owned()),
            match_id: active_match_id,
            ticket_id: None,
        }));
    }

    if let Some(ticket) = state
        .application()
        .current_matchmaking_ticket(user.user().id())?
    {
        return Ok(Json(UserActivityResponse {
            schema: "user_activity.v1",
            kind: "matchmaking",
            room_id: None,
            match_id: None,
            ticket_id: Some(ticket.id().as_str().to_owned()),
        }));
    }

    Ok(Json(UserActivityResponse {
        schema: "user_activity.v1",
        kind: "idle",
        room_id: None,
        match_id: None,
        ticket_id: None,
    }))
}

#[derive(Serialize)]
struct UserProfileDetailResponse {
    schema: &'static str,
    user: UserResponse,
    statistics: crate::PlayerStatistics,
}

async fn profile(
    State(state): State<AppState>,
    _viewer: AuthenticatedUser,
    Path(user_id): Path<String>,
) -> Result<Json<UserProfileDetailResponse>, ApiError> {
    let user_id = UserId::parse(user_id).map_err(|_| ApiError::invalid_id())?;
    let user = state.application().user(&user_id)?;
    let statistics = state.player_statistics(&user_id).await.map_err(|error| {
        tracing::error!(%error, %user_id, "failed to calculate player statistics");
        ApiError::internal()
    })?;
    Ok(Json(UserProfileDetailResponse {
        schema: "user_profile_detail.v1",
        user: UserResponse::from(&user),
        statistics,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateProfileRequest {
    nickname: String,
}

async fn update_profile(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    payload: Result<Json<UpdateProfileRequest>, JsonRejection>,
) -> Result<Json<UserResponse>, ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let user = state.application().update_profile(
        user.user().id(),
        UpdateProfile {
            nickname: payload.nickname,
        },
    )?;
    Ok(Json(UserResponse::from(&user)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdatePresentationRequest {
    character_id: String,
    outfit_id: String,
    avatar_path: String,
}

async fn update_presentation(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    payload: Result<Json<UpdatePresentationRequest>, JsonRejection>,
) -> Result<Json<UserResponse>, ApiError> {
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let user = state.application().update_presentation(
        user.user().id(),
        UpdatePresentation {
            character_id: payload.character_id,
            outfit_id: payload.outfit_id,
            avatar_path: payload.avatar_path,
        },
    )?;
    Ok(Json(UserResponse::from(&user)))
}
