use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use mahjong_core::MatchId;
use mamahjong_application::{GameCommand, SubmitGameCommand};
use serde::Deserialize;

use super::auth::AuthenticatedUser;
use super::dto::MatchViewResponse;
use super::error::ApiError;
use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/matches/{match_id}", get(get_match))
        .route("/matches/{match_id}/record", get(get_match_record))
        .route("/matches/{match_id}/commands", post(submit_command))
}

async fn get_match(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(match_id): Path<String>,
) -> Result<Json<MatchViewResponse>, ApiError> {
    let match_id = parse_match_id(match_id)?;
    let view = state
        .application()
        .match_view(user.user().id(), &match_id)?;
    Ok(Json(MatchViewResponse::from(&view)))
}

async fn get_match_record(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(match_id): Path<String>,
) -> Result<Json<mamahjong_application::MatchRecord>, ApiError> {
    let match_id = parse_match_id(match_id)?;
    let record = state
        .application()
        .match_record(user.user().id(), &match_id)?;
    Ok(Json(record))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRequest {
    expected_version: u64,
    command: CommandPayload,
}

#[derive(Deserialize)]
#[serde(tag = "name", content = "payload")]
enum CommandPayload {
    #[serde(rename = "riichi.discard")]
    Discard { tile_id: u16 },
    #[serde(rename = "riichi.riichi_discard")]
    RiichiDiscard { tile_id: u16 },
    #[serde(rename = "riichi.tsumo")]
    Tsumo,
    #[serde(rename = "riichi.pass")]
    Pass,
    #[serde(rename = "riichi.ron")]
    Ron,
    #[serde(rename = "riichi.chi")]
    Chi { tile_ids: [u16; 2] },
    #[serde(rename = "riichi.pon")]
    Pon { tile_ids: [u16; 2] },
    #[serde(rename = "riichi.open_kan")]
    OpenKan { tile_ids: [u16; 3] },
    #[serde(rename = "riichi.concealed_kan")]
    ConcealedKan { tile_ids: [u16; 4] },
    #[serde(rename = "riichi.added_kan")]
    AddedKan { meld_id: u8, tile_id: u16 },
    #[serde(rename = "riichi.nine_terminals")]
    NineTerminals,
}

impl From<CommandPayload> for GameCommand {
    fn from(value: CommandPayload) -> Self {
        match value {
            CommandPayload::Discard { tile_id } => Self::Discard { tile_id },
            CommandPayload::RiichiDiscard { tile_id } => Self::RiichiDiscard { tile_id },
            CommandPayload::Tsumo => Self::Tsumo,
            CommandPayload::Pass => Self::Pass,
            CommandPayload::Ron => Self::Ron,
            CommandPayload::Chi { tile_ids } => Self::Chi { tile_ids },
            CommandPayload::Pon { tile_ids } => Self::Pon { tile_ids },
            CommandPayload::OpenKan { tile_ids } => Self::OpenKan { tile_ids },
            CommandPayload::ConcealedKan { tile_ids } => Self::ConcealedKan { tile_ids },
            CommandPayload::AddedKan { meld_id, tile_id } => Self::AddedKan { meld_id, tile_id },
            CommandPayload::NineTerminals => Self::NineTerminals,
        }
    }
}

async fn submit_command(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(match_id): Path<String>,
    payload: Result<Json<CommandRequest>, JsonRejection>,
) -> Result<Json<MatchViewResponse>, ApiError> {
    let match_id = parse_match_id(match_id)?;
    let Json(payload) = payload.map_err(ApiError::invalid_json)?;
    let view = state.application().submit_game_command(
        user.user().id(),
        &match_id,
        SubmitGameCommand {
            expected_version: payload.expected_version,
            command: payload.command.into(),
        },
    )?;
    state
        .persist_match(user.user().id(), &match_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %match_id, "failed to persist match record");
            ApiError::internal()
        })?;
    Ok(Json(MatchViewResponse::from(&view)))
}

fn parse_match_id(value: String) -> Result<MatchId, ApiError> {
    MatchId::parse(value).map_err(|_| ApiError::invalid_id())
}
