use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use mamahjong_application::Character;
use serde::Serialize;

use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/characters", get(list))
        .route("/characters/default", get(default_character))
}

#[derive(Serialize)]
struct CharacterListResponse {
    schema: &'static str,
    characters: Vec<Character>,
}

async fn list(State(state): State<AppState>) -> Result<Json<CharacterListResponse>, ApiError> {
    let characters = state
        .application()
        .list_characters()?
        .into_iter()
        .filter(Character::enabled)
        .collect();
    Ok(Json(CharacterListResponse {
        schema: "character_list.v1",
        characters,
    }))
}

#[derive(Serialize)]
struct DefaultCharacterResponse {
    schema: &'static str,
    character: Character,
}

async fn default_character(
    State(state): State<AppState>,
) -> Result<Json<DefaultCharacterResponse>, ApiError> {
    Ok(Json(DefaultCharacterResponse {
        schema: "default_character.v1",
        character: state.application().default_character()?,
    }))
}
