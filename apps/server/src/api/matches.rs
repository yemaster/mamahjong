use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use mahjong_core::MatchId;
use mamahjong_application::{ErrorCode, GameCommand, MatchProjection, SubmitGameCommand};
use serde::Deserialize;
use serde_json::Value;

use super::auth::AuthenticatedUser;
use super::dto::MatchViewResponse;
use super::error::ApiError;
use crate::{AppState, AuditDraft};

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
        .match_projection(user.user().id(), &match_id)?;
    Ok(Json(MatchViewResponse::from_projection(
        &view,
        state.now_ms(),
        state.application(),
    )))
}

/// 一份牌谱。
///
/// 内存里只留着这个进程见过的对局，重启之后就空了；牌谱页要翻的多半是历史局，所以
/// 内存没命中就回落到归档目录。归档那条路同样要过座位校验，不在这局里坐过的人读不到。
async fn get_match_record(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(match_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let match_id = parse_match_id(match_id)?;
    let mut record = match state
        .application()
        .match_record(user.user().id(), &match_id)
    {
        Ok(record) => serde_json::to_value(record).map_err(|_| ApiError::internal())?,
        Err(error) if error.code() == ErrorCode::MatchNotFound => {
            let archived = state
                .archived_record(&match_id, user.user().id())
                .await
                .map_err(|error| {
                    tracing::error!(%error, %match_id, "failed to read archived match record");
                    ApiError::internal()
                })?;
            archived.ok_or_else(|| ApiError::from(error))?
        }
        Err(error) => return Err(ApiError::from(error)),
    };
    attach_rule_name(&mut record);
    Ok(Json(record))
}

/// 给牌谱补上一个读的时候才算出来的 `rule_name`。
///
/// 规则名是拿快照跟预设逐字段比出来的，不适合写死进归档：预设改版之后，磁盘上那份
/// 名字就成了旧账。列表接口（`archive::summarize`）走的是同一个函数，两处写出来的
/// 名字必然一致。
fn attach_rule_name(record: &mut Value) {
    let Some(name) = crate::archive::record_rule_name(record) else {
        return;
    };
    if let Some(object) = record.as_object_mut() {
        object.insert("rule_name".to_owned(), Value::String(name.to_owned()));
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRequest {
    expected_version: u64,
    command: CommandPayload,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "name", content = "payload", deny_unknown_fields)]
pub(crate) enum CommandPayload {
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
    #[serde(rename = "riichi.nuki")]
    Nuki { tile_id: u16 },
    #[serde(rename = "game.assets_ready")]
    MatchAssetsReady,
    #[serde(rename = "game.ready_for_hand", alias = "riichi.ready_for_hand")]
    ReadyForHand { hand_index: u32 },
    #[serde(rename = "game.settlement_played", alias = "riichi.settlement_played")]
    SettlementPlayed { hand_index: u32 },
    #[serde(
        rename = "game.confirm_settlement",
        alias = "riichi.confirm_settlement"
    )]
    ConfirmSettlement { hand_index: u32 },
    #[serde(rename = "game.request_exit_vote")]
    RequestExitVote,
    #[serde(rename = "game.vote_exit")]
    VoteExit { agree: bool },
    #[serde(rename = "impact.discard")]
    ImpactDiscard { tile_id: u16 },
    #[serde(rename = "impact.tsumo")]
    ImpactTsumo,
    #[serde(rename = "impact.ron")]
    ImpactRon,
    #[serde(rename = "impact.chi")]
    ImpactChi { tile_ids: [u16; 2] },
    #[serde(rename = "impact.pon")]
    ImpactPon,
    #[serde(rename = "impact.open_kan")]
    ImpactOpenKan,
    #[serde(rename = "impact.concealed_kan")]
    ImpactConcealedKan { tile_code: String },
    #[serde(rename = "impact.added_kan")]
    ImpactAddedKan { meld_id: u16 },
    #[serde(rename = "impact.indicator_concealed_kan")]
    ImpactIndicatorConcealedKan,
    #[serde(rename = "impact.pass")]
    ImpactPass,
    #[serde(rename = "impact.kan_animation_played")]
    ImpactKanAnimationPlayed { kan_id: u64 },
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
            CommandPayload::Nuki { tile_id } => Self::Nuki { tile_id },
            CommandPayload::MatchAssetsReady => Self::MatchAssetsReady,
            CommandPayload::ReadyForHand { hand_index } => Self::ReadyForHand { hand_index },
            CommandPayload::SettlementPlayed { hand_index } => {
                Self::SettlementPlayed { hand_index }
            }
            CommandPayload::ConfirmSettlement { hand_index } => {
                Self::ConfirmSettlement { hand_index }
            }
            CommandPayload::RequestExitVote => Self::RequestExitVote,
            CommandPayload::VoteExit { agree } => Self::VoteExit { agree },
            CommandPayload::ImpactDiscard { tile_id } => Self::ImpactDiscard { tile_id },
            CommandPayload::ImpactTsumo => Self::ImpactTsumo,
            CommandPayload::ImpactRon => Self::ImpactRon,
            CommandPayload::ImpactChi { tile_ids } => Self::ImpactChi { tile_ids },
            CommandPayload::ImpactPon => Self::ImpactPon,
            CommandPayload::ImpactOpenKan => Self::ImpactOpenKan,
            CommandPayload::ImpactConcealedKan { tile_code } => {
                Self::ImpactConcealedKan { tile_code }
            }
            CommandPayload::ImpactAddedKan { meld_id } => Self::ImpactAddedKan { meld_id },
            CommandPayload::ImpactIndicatorConcealedKan => Self::ImpactIndicatorConcealedKan,
            CommandPayload::ImpactPass => Self::ImpactPass,
            CommandPayload::ImpactKanAnimationPlayed { kan_id } => {
                Self::ImpactKanAnimationPlayed { kan_id }
            }
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
    let view = apply_command(
        &state,
        user.user().id(),
        &match_id,
        SubmitGameCommand {
            expected_version: payload.expected_version,
            command: payload.command.into(),
        },
    )
    .await?;
    Ok(Json(MatchViewResponse::from_projection(
        &view,
        state.now_ms(),
        state.application(),
    )))
}

/// Applies a command, archives the match, and wakes the realtime stream.
///
/// HTTP and WebSocket share this path so both produce the same event sequence.
pub(crate) async fn apply_command(
    state: &AppState,
    actor: &mahjong_core::UserId,
    match_id: &MatchId,
    command: SubmitGameCommand,
) -> Result<MatchProjection, ApiError> {
    let view = state
        .application()
        .submit_game(actor, match_id, command, state.now_ms())?;
    announce_advance(
        state,
        actor,
        match_id,
        view.version(),
        view.event_sequence(),
        view.has_result() || view.terminated_by_exit_vote(),
    )
    .await?;
    Ok(view)
}

/// Archives, audits and wakes the stream after a match advanced.
///
/// Player commands and the clock sweeper share it so a timeout leaves exactly
/// the same trail as a played tile. Failures are logged where they happen.
pub(crate) async fn announce_advance(
    state: &AppState,
    actor: &mahjong_core::UserId,
    match_id: &MatchId,
    version: u64,
    latest_sequence: u64,
    finished: bool,
) -> Result<(), ApiError> {
    state
        .persist_match(actor, match_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %match_id, "failed to persist match record");
            ApiError::internal()
        })?;
    if finished {
        state
            .record_audit(AuditDraft {
                severity: "info",
                category: "game",
                action: "game.finished",
                actor_id: Some(actor.as_str().to_owned()),
                target_type: "match",
                target_id: Some(match_id.as_str().to_owned()),
                outcome: "success",
                detail: "对局已结束".to_owned(),
            })
            .await
            .map_err(|error| {
                tracing::error!(%error, %match_id, "failed to record match audit");
                ApiError::internal()
            })?;
    }
    state.realtime().publish(
        &super::realtime::match_stream(match_id),
        super::realtime::StreamNotice {
            version,
            latest_sequence,
        },
    );
    Ok(())
}

fn parse_match_id(value: String) -> Result<MatchId, ApiError> {
    MatchId::parse(value).map_err(|_| ApiError::invalid_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 不带参数的指令一律不能再带 `payload` 字段。
    ///
    /// 这几条在指令枚举里是单元变体，多给一个空对象 serde 就整帧解析失败，
    /// 走实时连接时表现为「按钮点了什么都没发生」。网页端曾经给退出投票发过
    /// `payload: {}`，这条测试就是钉住那次回归。
    #[test]
    fn commands_without_arguments_parse_without_a_payload_field() {
        for name in [
            "riichi.tsumo",
            "riichi.pass",
            "riichi.ron",
            "riichi.nine_terminals",
            "game.request_exit_vote",
            "impact.tsumo",
            "impact.ron",
            "impact.pon",
            "impact.open_kan",
            "impact.indicator_concealed_kan",
            "impact.pass",
        ] {
            let frame = format!(r#"{{"name":"{name}"}}"#);
            serde_json::from_str::<CommandPayload>(&frame)
                .unwrap_or_else(|error| panic!("{name} should parse without a payload: {error}"));
            assert!(
                serde_json::from_str::<CommandPayload>(&format!(
                    r#"{{"name":"{name}","payload":{{}}}}"#
                ))
                .is_err(),
                "{name} must not be sent with an empty payload object",
            );
        }
    }

    #[test]
    fn shared_presentation_commands_use_game_namespace_and_accept_legacy_names() {
        for (current, legacy) in [
            ("game.ready_for_hand", "riichi.ready_for_hand"),
            ("game.settlement_played", "riichi.settlement_played"),
            ("game.confirm_settlement", "riichi.confirm_settlement"),
        ] {
            for name in [current, legacy] {
                let frame = format!(r#"{{"name":"{name}","payload":{{"hand_index":7}}}}"#);
                assert!(
                    serde_json::from_str::<CommandPayload>(&frame).is_ok(),
                    "{name} should remain accepted",
                );
            }
        }
    }

    #[test]
    fn command_payloads_reject_unrecognized_fields() {
        for frame in [
            r#"{"name":"impact.ron","force":true}"#,
            r#"{"name":"impact.chi","payload":{"tile_ids":[1,2],"force":true}}"#,
            r#"{"name":"impact.discard","payload":{"tile_id":1,"seat":2}}"#,
        ] {
            assert!(
                serde_json::from_str::<CommandPayload>(frame).is_err(),
                "unrecognized command data must be rejected: {frame}",
            );
        }
    }
}
