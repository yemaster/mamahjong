use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use mamahjong_application::{RegisterUser, UpdateProfile};
use serde::Deserialize;

use super::auth::AuthenticatedUser;
use super::dto::{AuthResponse, UserResponse};
use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/registrations", post(register))
        .route("/sessions", post(login))
        .route("/users/me", get(me))
        .route("/users/me/profile", patch(update_profile))
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
    let (user, session) = tokio::task::spawn_blocking(move || {
        application.login(&payload.login_name, &payload.password)
    })
    .await
    .map_err(|_| ApiError::internal())??;
    Ok(Json(AuthResponse::new(&user, &session)))
}

async fn me(user: AuthenticatedUser) -> Json<UserResponse> {
    Json(UserResponse::from(user.user()))
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
