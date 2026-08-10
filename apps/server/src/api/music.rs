use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use mamahjong_application::{MusicTrack, UpdateMusic};
use serde::{Deserialize, Serialize};

use super::auth::AuthenticatedUser;
use super::dto::UserResponse;
use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/music-tracks", get(list))
        .route("/users/me/music", axum::routing::put(update))
}

#[derive(Serialize)]
struct MusicTrackListResponse {
    schema: &'static str,
    music_tracks: Vec<MusicTrack>,
}

async fn list(State(state): State<AppState>) -> Result<Json<MusicTrackListResponse>, ApiError> {
    let music_tracks = state
        .application()
        .list_music_tracks()?
        .into_iter()
        .filter(MusicTrack::enabled)
        .collect();
    Ok(Json(MusicTrackListResponse {
        schema: "music_track_list.v1",
        music_tracks,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMusicRequest {
    #[serde(default)]
    lobby_music_id: Option<String>,
    #[serde(default)]
    match_music_id: Option<String>,
    #[serde(default)]
    riichi_music_id: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<UpdateMusicRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state.application().update_music(
        user.user().id(),
        UpdateMusic {
            lobby_music_id: request.lobby_music_id,
            match_music_id: request.match_music_id,
            riichi_music_id: request.riichi_music_id,
        },
    )?;
    Ok(Json(UserResponse::from(&user)))
}
