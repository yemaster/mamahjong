use axum::Router;
use axum::extract::{Json, State};
use axum::routing::get;
use serde::Serialize;

use super::auth::AuthenticatedUser;
use super::error::ApiError;
use crate::{AppState, MatchRecordSummary};

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/records", get(list_records))
}

#[derive(Serialize)]
struct RecordListResponse {
    schema: &'static str,
    records: Vec<MatchRecordSummary>,
}

/// 当前用户打完的对局，按开局时间倒序。
///
/// 只看自己的：牌谱里有别人的手牌和牌山，不能按对局编号随便翻。
async fn list_records(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<RecordListResponse>, ApiError> {
    let user_id = user.user().id();
    let records = state.player_records(user_id).await.map_err(|error| {
        tracing::error!(%error, %user_id, "failed to list player match records");
        ApiError::internal()
    })?;
    Ok(Json(RecordListResponse {
        schema: "match_record_list.v1",
        records,
    }))
}
