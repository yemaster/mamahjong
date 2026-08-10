use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use mamahjong_application::{Tablecloth, UpdateTablecloth};
use serde::{Deserialize, Serialize};

use super::auth::AuthenticatedUser;
use super::dto::UserResponse;
use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/tablecloths", get(list))
        .route("/users/me/tablecloth", axum::routing::put(update))
}

#[derive(Serialize)]
struct TableclothListResponse {
    schema: &'static str,
    tablecloths: Vec<Tablecloth>,
}

async fn list(State(state): State<AppState>) -> Result<Json<TableclothListResponse>, ApiError> {
    let tablecloths = state
        .application()
        .list_tablecloths()?
        .into_iter()
        .filter(Tablecloth::enabled)
        .collect();
    Ok(Json(TableclothListResponse {
        schema: "tablecloth_list.v1",
        tablecloths,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateTableclothRequest {
    tablecloth_id: String,
}

async fn update(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<UpdateTableclothRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state.application().update_tablecloth(
        user.user().id(),
        UpdateTablecloth {
            tablecloth_id: request.tablecloth_id,
        },
    )?;
    Ok(Json(UserResponse::from(&user)))
}
