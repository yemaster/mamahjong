use axum::extract::{Path, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use mahjong_core::{RoomId, UserId};
use mamahjong_application::{
    AccountRole, AccountStatus, ApplicationError, Character, ErrorCode, RoomLifecycle,
    RoomVisibility, SaveCharacter, SaveMusicTrack, SaveTablecloth, Tablecloth, UpdateProfile, User,
};
use serde::{Deserialize, Serialize};

use super::{AdminSessionError, AdminSessionView, AdminSessions};
use crate::{AppState, AuditDraft, AuditError, AuditEvent};

const SESSION_COOKIE: &str = "mamahjong_admin_session";
const CSRF_HEADER: &str = "x-csrf-token";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/session",
            get(session_bootstrap).post(login).delete(logout),
        )
        .route("/me", get(me))
        .route("/overview", get(overview))
        .route("/users", get(users))
        .route("/users/{user_id}", put(update_user))
        .route("/users/{user_id}/status", put(update_user_status))
        .route("/rooms", get(rooms))
        .route("/rooms/{room_id}/close", post(close_room))
        .route("/matches", get(matches))
        .route("/matches/{match_id}", get(match_detail))
        .route("/characters", get(characters).post(create_character))
        .route(
            "/characters/{character_id}",
            put(update_character).delete(delete_character),
        )
        .route("/tablecloths", get(tablecloths).post(create_tablecloth))
        .route(
            "/tablecloths/{tablecloth_id}",
            put(update_tablecloth).delete(delete_tablecloth),
        )
        .route("/music", get(music).post(create_music))
        .route("/music/{music_id}", put(update_music).delete(delete_music))
        .route("/database", get(database))
        .route("/audit", get(audit))
}

#[derive(Serialize)]
struct CharacterListResponse {
    schema: &'static str,
    characters: Vec<Character>,
}

async fn characters(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CharacterListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(CharacterListResponse {
        schema: "admin_character_list.v1",
        characters: state.application().list_characters()?,
    }))
}

async fn create_character(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveCharacter>,
) -> Result<(StatusCode, Json<Character>), AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if state
        .application()
        .list_characters()?
        .iter()
        .any(|character| character.id() == request.id)
    {
        return Err(AdminApiError::conflict(
            "character.already_exists",
            "角色编号已存在",
        ));
    }
    let character = state.application().save_character(request)?;
    record_character_audit(
        &state,
        &authenticated.user,
        "admin.character.created",
        &character,
        "角色已添加",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(character)))
}

async fn update_character(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(character_id): Path<String>,
    Json(mut request): Json<SaveCharacter>,
) -> Result<Json<Character>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if character_id != request.id {
        return Err(AdminApiError::invalid_id());
    }
    request.id = character_id;
    let character = state.application().save_character(request)?;
    record_character_audit(
        &state,
        &authenticated.user,
        "admin.character.updated",
        &character,
        "角色资料已更新",
    )
    .await?;
    Ok(Json(character))
}

async fn delete_character(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(character_id): Path<String>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    state.application().delete_character(&character_id)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.character.deleted",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "character",
            target_id: Some(character_id),
            outcome: "success",
            detail: "角色已删除".to_owned(),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_character_audit(
    state: &AppState,
    actor: &User,
    action: &'static str,
    character: &Character,
    detail: &'static str,
) -> Result<(), AdminApiError> {
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action,
            actor_id: Some(actor.id().as_str().to_owned()),
            target_type: "character",
            target_id: Some(character.id().to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        })
        .await?;
    Ok(())
}

#[derive(Serialize)]
struct TableclothListResponse {
    schema: &'static str,
    tablecloths: Vec<Tablecloth>,
}

async fn tablecloths(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TableclothListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(TableclothListResponse {
        schema: "admin_tablecloth_list.v1",
        tablecloths: state.application().list_tablecloths()?,
    }))
}

async fn create_tablecloth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveTablecloth>,
) -> Result<(StatusCode, Json<Tablecloth>), AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if state
        .application()
        .list_tablecloths()?
        .iter()
        .any(|tablecloth| tablecloth.id() == request.id)
    {
        return Err(AdminApiError::conflict(
            "tablecloth.already_exists",
            "桌布编号已存在",
        ));
    }
    let tablecloth = state.application().save_tablecloth(request)?;
    record_tablecloth_audit(
        &state,
        &authenticated.user,
        "admin.tablecloth.created",
        &tablecloth,
        "桌布已添加",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(tablecloth)))
}

async fn update_tablecloth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tablecloth_id): Path<String>,
    Json(mut request): Json<SaveTablecloth>,
) -> Result<Json<Tablecloth>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if tablecloth_id != request.id {
        return Err(AdminApiError::invalid_id());
    }
    request.id = tablecloth_id;
    let tablecloth = state.application().save_tablecloth(request)?;
    record_tablecloth_audit(
        &state,
        &authenticated.user,
        "admin.tablecloth.updated",
        &tablecloth,
        "桌布资料已更新",
    )
    .await?;
    Ok(Json(tablecloth))
}

async fn delete_tablecloth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tablecloth_id): Path<String>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    state.application().delete_tablecloth(&tablecloth_id)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.tablecloth.deleted",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "tablecloth",
            target_id: Some(tablecloth_id),
            outcome: "success",
            detail: "桌布已删除".to_owned(),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_tablecloth_audit(
    state: &AppState,
    actor: &User,
    action: &'static str,
    tablecloth: &Tablecloth,
    detail: &'static str,
) -> Result<(), AdminApiError> {
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action,
            actor_id: Some(actor.id().as_str().to_owned()),
            target_type: "tablecloth",
            target_id: Some(tablecloth.id().to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        })
        .await?;
    Ok(())
}

#[derive(Serialize)]
struct MusicListResponse {
    schema: &'static str,
    music_tracks: Vec<mamahjong_application::MusicTrack>,
}

async fn music(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MusicListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(MusicListResponse {
        schema: "admin_music_list.v1",
        music_tracks: state.application().list_music_tracks()?,
    }))
}

async fn create_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveMusicTrack>,
) -> Result<(StatusCode, Json<mamahjong_application::MusicTrack>), AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if state
        .application()
        .list_music_tracks()?
        .iter()
        .any(|track| track.id() == request.id)
    {
        return Err(AdminApiError::conflict(
            "music.already_exists",
            "音乐编号已存在",
        ));
    }
    let track = state.application().save_music_track(request)?;
    record_music_audit(
        &state,
        &authenticated.user,
        "admin.music.created",
        track.id(),
        "音乐已添加",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(track)))
}

async fn update_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(music_id): Path<String>,
    Json(mut request): Json<SaveMusicTrack>,
) -> Result<Json<mamahjong_application::MusicTrack>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if music_id != request.id {
        return Err(AdminApiError::invalid_id());
    }
    request.id = music_id;
    let track = state.application().save_music_track(request)?;
    record_music_audit(
        &state,
        &authenticated.user,
        "admin.music.updated",
        track.id(),
        "音乐资料已更新",
    )
    .await?;
    Ok(Json(track))
}

async fn delete_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(music_id): Path<String>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    state.application().delete_music_track(&music_id)?;
    record_music_audit(
        &state,
        &authenticated.user,
        "admin.music.deleted",
        &music_id,
        "音乐已删除",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_music_audit(
    state: &AppState,
    actor: &User,
    action: &'static str,
    id: &str,
    detail: &'static str,
) -> Result<(), AdminApiError> {
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action,
            actor_id: Some(actor.id().as_str().to_owned()),
            target_type: "music",
            target_id: Some(id.to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        })
        .await?;
    Ok(())
}

#[derive(Serialize)]
struct MatchListResponse {
    schema: &'static str,
    matches: Vec<crate::MatchRecordSummary>,
}

async fn matches(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MatchListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(MatchListResponse {
        schema: "admin_match_list.v1",
        matches: state
            .admin_records()
            .await
            .map_err(|_| AdminApiError::internal())?,
    }))
}

async fn match_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(match_id): Path<String>,
) -> Result<Json<serde_json::Value>, AdminApiError> {
    authenticate(&state, &headers)?;
    state
        .admin_archived_record(&match_id)
        .await
        .map_err(|_| AdminApiError::internal())?
        .map(Json)
        .ok_or_else(|| AdminApiError::not_found("match.not_found", "对局不存在"))
}

#[derive(Serialize)]
struct DatabaseTableResponse {
    name: &'static str,
    label: &'static str,
    records: usize,
    writable: bool,
}

#[derive(Serialize)]
struct DatabaseResponse {
    schema: &'static str,
    engine: &'static str,
    persistent: bool,
    status: &'static str,
    tables: Vec<DatabaseTableResponse>,
}

async fn database(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DatabaseResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    let application = state.application();
    let records = state
        .admin_records()
        .await
        .map_err(|_| AdminApiError::internal())?;
    Ok(Json(DatabaseResponse {
        schema: "admin_database.v1",
        engine: if application.persistence_enabled() {
            "PostgreSQL"
        } else {
            "内存"
        },
        persistent: application.persistence_enabled(),
        status: "正常",
        tables: vec![
            DatabaseTableResponse {
                name: "mamahjong_users",
                label: "用户",
                records: application.list_users()?.len(),
                writable: true,
            },
            DatabaseTableResponse {
                name: "mamahjong_characters",
                label: "角色",
                records: application.list_characters()?.len(),
                writable: true,
            },
            DatabaseTableResponse {
                name: "mamahjong_tablecloths",
                label: "桌布",
                records: application.list_tablecloths()?.len(),
                writable: true,
            },
            DatabaseTableResponse {
                name: "mamahjong_music_tracks",
                label: "音乐",
                records: application.list_music_tracks()?.len(),
                writable: true,
            },
            DatabaseTableResponse {
                name: "match_archive",
                label: "对局归档",
                records: records.len(),
                writable: false,
            },
            DatabaseTableResponse {
                name: "audit_log",
                label: "审计日志",
                records: state.audit().recent(500)?.len(),
                writable: false,
            },
        ],
    }))
}

#[derive(Serialize)]
struct SessionBootstrapResponse {
    schema: &'static str,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    login_csrf: Option<String>,
}

async fn session_bootstrap(State(state): State<AppState>) -> Json<SessionBootstrapResponse> {
    let enabled = state.admin_sessions().is_enabled();
    Json(SessionBootstrapResponse {
        schema: "admin_session_bootstrap.v1",
        enabled,
        login_csrf: state.admin_sessions().login_csrf().map(str::to_owned),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    login_name: String,
    password: String,
    login_csrf: String,
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, AdminApiError> {
    let expected = state
        .admin_sessions()
        .login_csrf()
        .ok_or_else(AdminApiError::disabled)?;
    if !AdminSessions::csrf_matches(expected, &request.login_csrf) {
        audit_auth_failure(&state, "admin.auth.csrf_rejected", "CSRF 校验失败").await;
        return Err(AdminApiError::csrf());
    }
    let user = match state
        .application()
        .verify_login(&request.login_name, &request.password)
    {
        Ok(user) if user.role() == AccountRole::Administrator => user,
        Ok(_) | Err(_) => {
            audit_auth_failure(&state, "admin.auth.login_failed", "登录失败").await;
            return Err(AdminApiError::credentials());
        }
    };
    let session = state.admin_sessions().create(user.id().clone())?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "auth",
            action: "admin.auth.login_succeeded",
            actor_id: Some(user.id().as_str().to_owned()),
            target_type: "session",
            target_id: None,
            outcome: "success",
            detail: "管理端登录成功".to_owned(),
        })
        .await?;
    let mut response = Json(identity_response(&user, &session.csrf_token)).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        session_cookie(&session.token, state.admin_sessions().cookie_secure())?,
    );
    Ok(response)
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "auth",
            action: "admin.auth.logout",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "session",
            target_id: None,
            outcome: "success",
            detail: "管理端退出".to_owned(),
        })
        .await?;
    state
        .admin_sessions()
        .remove(&authenticated.session.token)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        expired_session_cookie(state.admin_sessions().cookie_secure())?,
    );
    Ok(response)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<IdentityResponse>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    Ok(Json(identity_response(
        &authenticated.user,
        &authenticated.session.csrf_token,
    )))
}

#[derive(Serialize)]
struct IdentityResponse {
    schema: &'static str,
    id: String,
    login_name: String,
    nickname: String,
    csrf_token: String,
}

fn identity_response(user: &User, csrf_token: &str) -> IdentityResponse {
    IdentityResponse {
        schema: "admin_identity.v1",
        id: user.id().as_str().to_owned(),
        login_name: user.login_name().to_owned(),
        nickname: user.profile().nickname().as_str().to_owned(),
        csrf_token: csrf_token.to_owned(),
    }
}

#[derive(Serialize)]
struct OverviewResponse {
    schema: &'static str,
    user_count: usize,
    waiting_room_count: usize,
    playing_room_count: usize,
    match_count: usize,
    character_count: usize,
    tablecloth_count: usize,
    music_count: usize,
    recent_audit: Vec<AuditEvent>,
}

async fn overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OverviewResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    let users = state.application().list_users()?;
    let rooms = state.application().list_all_rooms()?;
    let match_count = state
        .admin_records()
        .await
        .map_err(|_| AdminApiError::internal())?
        .len();
    Ok(Json(OverviewResponse {
        schema: "admin_overview.v1",
        user_count: users.len(),
        waiting_room_count: rooms
            .iter()
            .filter(|room| room.lifecycle() == RoomLifecycle::Waiting)
            .count(),
        playing_room_count: rooms
            .iter()
            .filter(|room| room.lifecycle() == RoomLifecycle::Playing)
            .count(),
        match_count,
        character_count: state.application().list_characters()?.len(),
        tablecloth_count: state.application().list_tablecloths()?.len(),
        music_count: state.application().list_music_tracks()?.len(),
        recent_audit: state.audit().recent(10)?,
    }))
}

#[derive(Serialize)]
struct UserListResponse {
    schema: &'static str,
    users: Vec<AdminUserResponse>,
}

#[derive(Serialize)]
struct AdminUserResponse {
    id: String,
    version: u64,
    login_name: String,
    nickname: String,
    status: &'static str,
    role: &'static str,
}

async fn users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(UserListResponse {
        schema: "admin_user_list.v1",
        users: state
            .application()
            .list_users()?
            .iter()
            .map(admin_user_response)
            .collect(),
    }))
}

fn admin_user_response(user: &User) -> AdminUserResponse {
    AdminUserResponse {
        id: user.id().as_str().to_owned(),
        version: user.version(),
        login_name: user.login_name().to_owned(),
        nickname: user.profile().nickname().as_str().to_owned(),
        status: match user.status() {
            AccountStatus::Active => "active",
            AccountStatus::Suspended => "suspended",
        },
        role: match user.role() {
            AccountRole::Player => "player",
            AccountRole::Administrator => "administrator",
        },
    }
}

async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<AdminUserResponse>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    let user_id = UserId::parse(user_id).map_err(|_| AdminApiError::invalid_id())?;
    let user = state.application().update_profile(
        &user_id,
        UpdateProfile {
            nickname: request.nickname,
        },
    )?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.user.updated",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "user",
            target_id: Some(user_id.as_str().to_owned()),
            outcome: "success",
            detail: "用户资料已更新".to_owned(),
        })
        .await?;
    Ok(Json(admin_user_response(&user)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateUserRequest {
    nickname: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateUserStatusRequest {
    status: AdminAccountStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdminAccountStatus {
    Active,
    Suspended,
}

async fn update_user_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserStatusRequest>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    let user_id = UserId::parse(user_id).map_err(|_| AdminApiError::invalid_id())?;
    if user_id == *authenticated.user.id() {
        return Err(AdminApiError::conflict(
            "admin.current_user",
            "不能停用当前账号",
        ));
    }
    let status = match request.status {
        AdminAccountStatus::Active => AccountStatus::Active,
        AdminAccountStatus::Suspended => AccountStatus::Suspended,
    };
    state.application().set_user_status(&user_id, status)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: match status {
                AccountStatus::Active => "admin.user.activated",
                AccountStatus::Suspended => "admin.user.suspended",
            },
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "user",
            target_id: Some(user_id.as_str().to_owned()),
            outcome: "success",
            detail: match status {
                AccountStatus::Active => "账号已恢复",
                AccountStatus::Suspended => "账号已停用",
            }
            .to_owned(),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct RoomListResponse {
    schema: &'static str,
    rooms: Vec<AdminRoomResponse>,
}

#[derive(Serialize)]
struct AdminRoomResponse {
    id: String,
    version: u64,
    name: String,
    owner_user_id: String,
    visibility: &'static str,
    lifecycle: &'static str,
    member_count: usize,
    seat_count: u8,
    active_match_id: Option<String>,
}

async fn rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RoomListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    let rooms = state.application().list_all_rooms()?;
    Ok(Json(RoomListResponse {
        schema: "admin_room_list.v1",
        rooms: rooms
            .iter()
            .map(|room| AdminRoomResponse {
                id: room.id().as_str().to_owned(),
                version: room.version(),
                name: room.name().to_owned(),
                owner_user_id: room.owner_user_id().as_str().to_owned(),
                visibility: match room.visibility() {
                    RoomVisibility::Public => "public",
                    RoomVisibility::Private => "private",
                },
                lifecycle: match room.lifecycle() {
                    RoomLifecycle::Waiting => "waiting",
                    RoomLifecycle::Playing => "playing",
                    RoomLifecycle::Closed => "closed",
                },
                member_count: room.members().len(),
                seat_count: room.rule_snapshot().seat_count(),
                active_match_id: room.active_match_id().map(|id| id.as_str().to_owned()),
            })
            .collect(),
    }))
}

async fn close_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    let room_id = RoomId::parse(room_id).map_err(|_| AdminApiError::invalid_id())?;
    state.application().close_room_by_administrator(&room_id)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.room.closed",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "room",
            target_id: Some(room_id.as_str().to_owned()),
            outcome: "success",
            detail: "房间已关闭".to_owned(),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct AuditListResponse {
    schema: &'static str,
    events: Vec<AuditEvent>,
}

async fn audit(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuditListResponse>, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.audit.viewed",
            actor_id: Some(authenticated.user.id().as_str().to_owned()),
            target_type: "audit_log",
            target_id: None,
            outcome: "success",
            detail: "查看审计日志".to_owned(),
        })
        .await?;
    Ok(Json(AuditListResponse {
        schema: "admin_audit_list.v1",
        events: state.audit().recent(500)?,
    }))
}

struct AuthenticatedAdmin {
    user: User,
    session: AdminSessionView,
}

fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedAdmin, AdminApiError> {
    let token = cookie_value(headers, SESSION_COOKIE).ok_or_else(AdminApiError::unauthorized)?;
    let session = state
        .admin_sessions()
        .authenticate(token)?
        .ok_or_else(AdminApiError::unauthorized)?;
    let user = state.application().user(&session.user_id)?;
    if user.status() != AccountStatus::Active || user.role() != AccountRole::Administrator {
        state.admin_sessions().remove(token)?;
        return Err(AdminApiError::unauthorized());
    }
    Ok(AuthenticatedAdmin { user, session })
}

fn require_csrf(
    headers: &HeaderMap,
    authenticated: &AuthenticatedAdmin,
) -> Result<(), AdminApiError> {
    let actual = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(AdminApiError::csrf)?;
    if !AdminSessions::csrf_matches(&authenticated.session.csrf_token, actual) {
        return Err(AdminApiError::csrf());
    }
    Ok(())
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name == name).then_some(value))
}

fn session_cookie(token: &str, secure: bool) -> Result<HeaderValue, AdminApiError> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=28800{secure}"
    ))
    .map_err(|_| AdminApiError::internal())
}

fn expired_session_cookie(secure: bool) -> Result<HeaderValue, AdminApiError> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"
    ))
    .map_err(|_| AdminApiError::internal())
}

async fn audit_auth_failure(state: &AppState, action: &'static str, detail: &'static str) {
    let _ = state
        .record_audit(AuditDraft {
            severity: "warn",
            category: "auth",
            action,
            actor_id: None,
            target_type: "session",
            target_id: None,
            outcome: "failure",
            detail: detail.to_owned(),
        })
        .await;
}

#[derive(Debug)]
struct AdminApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl AdminApiError {
    fn disabled() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "admin.disabled",
            message: "管理端未启用",
        }
    }

    fn credentials() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "admin.invalid_credentials",
            message: "账号或密码错误",
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "admin.unauthorized",
            message: "请重新登录",
        }
    }

    fn csrf() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "admin.csrf",
            message: "请求校验失败",
        }
    }

    fn invalid_id() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "request.invalid_id",
            message: "资源 ID 无效",
        }
    }

    fn conflict(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message,
        }
    }

    fn not_found(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message,
        }
    }

    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "server.internal",
            message: "服务器内部错误",
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    kind: &'static str,
    schema: &'static str,
    code: &'static str,
    message: &'static str,
    retryable: bool,
}

impl IntoResponse for AdminApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                kind: "error",
                schema: "error.v1",
                code: self.code,
                message: self.message,
                retryable: self.status.is_server_error(),
            }),
        )
            .into_response()
    }
}

impl From<AdminSessionError> for AdminApiError {
    fn from(_: AdminSessionError) -> Self {
        Self::internal()
    }
}

impl From<AuditError> for AdminApiError {
    fn from(_: AuditError) -> Self {
        Self::internal()
    }
}

impl From<ApplicationError> for AdminApiError {
    fn from(error: ApplicationError) -> Self {
        match error.code() {
            ErrorCode::InvalidCredentials | ErrorCode::InvalidSession => Self::unauthorized(),
            ErrorCode::UserUnavailable => Self {
                status: StatusCode::NOT_FOUND,
                code: "admin.resource_not_found",
                message: "资源不存在",
            },
            ErrorCode::RoomNotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: "room.not_found",
                message: "房间不存在",
            },
            ErrorCode::RoomPlaying | ErrorCode::RoomClosed => {
                Self::conflict("room.not_waiting", "只能关闭等待中的房间")
            }
            ErrorCode::InvalidCharacter => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "request.invalid_character",
                message: "角色资料格式不正确",
            },
            ErrorCode::CharacterNotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: "character.not_found",
                message: "角色不存在",
            },
            ErrorCode::CharacterDefaultRequired => {
                Self::conflict("character.default_required", "不能删除初始角色")
            }
            ErrorCode::InvalidTablecloth => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "request.invalid_tablecloth",
                message: "桌布资料格式不正确",
            },
            ErrorCode::TableclothNotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: "tablecloth.not_found",
                message: "桌布不存在",
            },
            ErrorCode::TableclothDefaultRequired => {
                Self::conflict("tablecloth.default_required", "不能删除初始桌布")
            }
            ErrorCode::InvalidMusicTrack => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "request.invalid_music_track",
                message: "音乐资料格式不正确",
            },
            ErrorCode::MusicTrackNotFound => Self::not_found("music.not_found", "音乐不存在"),
            ErrorCode::MusicTrackDefaultRequired => {
                Self::conflict("music.default_required", "不能删除默认音乐")
            }
            _ => Self::internal(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use axum::body::{Body, to_bytes};
    use axum::http::header::{COOKIE, SET_COOKIE};
    use axum::http::{HeaderMap, Method, Request, StatusCode};
    use mamahjong_application::{AccountStatus, RegisterUser};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::CSRF_HEADER;
    use crate::{AppState, build_router};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn admin_state() -> (AppState, std::path::PathBuf) {
        let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mamahjong-admin-api-{}-{serial}",
            std::process::id()
        ));
        let state = AppState::persistent_with_admin(&directory, false).expect("admin state");
        state
            .application()
            .bootstrap_administrator(RegisterUser {
                login_name: "operator".to_owned(),
                password: "correct horse battery staple".to_owned(),
                nickname: "运营人员".to_owned(),
            })
            .expect("administrator");
        (state, directory)
    }

    async fn request(
        router: axum::Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
        csrf: Option<&str>,
    ) -> (StatusCode, HeaderMap, Option<Value>) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(COOKIE, cookie);
        }
        if let Some(csrf) = csrf {
            builder = builder.header(CSRF_HEADER, csrf);
        }
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(&value).expect("JSON"))
            }
            None => Body::empty(),
        };
        let response = router
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1_048_576)
            .await
            .expect("body");
        let body =
            (!bytes.is_empty()).then(|| serde_json::from_slice(&bytes).expect("JSON response"));
        (status, headers, body)
    }

    async fn login(router: axum::Router) -> (String, String) {
        let (_, _, bootstrap) = request(
            router.clone(),
            Method::GET,
            "/api/v1/admin/session",
            None,
            None,
            None,
        )
        .await;
        let login_csrf = bootstrap.expect("bootstrap")["login_csrf"]
            .as_str()
            .expect("login CSRF")
            .to_owned();
        let (status, headers, identity) = request(
            router,
            Method::POST,
            "/api/v1/admin/session",
            Some(json!({
                "login_name": "operator",
                "password": "correct horse battery staple",
                "login_csrf": login_csrf
            })),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let set_cookie = headers
            .get(SET_COOKIE)
            .expect("session cookie")
            .to_str()
            .expect("cookie text");
        let cookie = set_cookie
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned();
        let csrf = identity.expect("identity")["csrf_token"]
            .as_str()
            .expect("CSRF")
            .to_owned();
        (cookie, csrf)
    }

    #[tokio::test]
    async fn administrator_session_requires_role_and_csrf() {
        let (state, directory) = admin_state();
        state
            .application()
            .register(RegisterUser {
                login_name: "player".to_owned(),
                password: "correct horse battery staple".to_owned(),
                nickname: "普通玩家".to_owned(),
            })
            .expect("player");
        let router = build_router(state);
        let (_, _, bootstrap) = request(
            router.clone(),
            Method::GET,
            "/api/v1/admin/session",
            None,
            None,
            None,
        )
        .await;
        let login_csrf = bootstrap.expect("bootstrap")["login_csrf"]
            .as_str()
            .expect("login CSRF")
            .to_owned();
        let (status, _, _) = request(
            router.clone(),
            Method::POST,
            "/api/v1/admin/session",
            Some(json!({
                "login_name": "player",
                "password": "correct horse battery staple",
                "login_csrf": login_csrf
            })),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (cookie, _) = login(router.clone()).await;
        let (status, _, _) = request(
            router,
            Method::PUT,
            "/api/v1/admin/users/user_018f22e2-7c30-7cc4-98c4-dc0c0c07398f/status",
            Some(json!({"status": "suspended"})),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn user_status_changes_and_audit_are_queryable() {
        let (state, directory) = admin_state();
        let (player, _) = state
            .application()
            .register(RegisterUser {
                login_name: "target".to_owned(),
                password: "correct horse battery staple".to_owned(),
                nickname: "目标玩家".to_owned(),
            })
            .expect("player");
        let router = build_router(state.clone());
        let (cookie, csrf) = login(router.clone()).await;
        let (status, _, _) = request(
            router.clone(),
            Method::PUT,
            &format!("/api/v1/admin/users/{}/status", player.id().as_str()),
            Some(json!({"status": "suspended"})),
            Some(&cookie),
            Some(&csrf),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            state
                .application()
                .user(player.id())
                .expect("user")
                .status(),
            AccountStatus::Suspended
        );
        let (status, _, audit) = request(
            router,
            Method::GET,
            "/api/v1/admin/audit",
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            audit.expect("audit")["events"]
                .as_array()
                .expect("events")
                .iter()
                .any(|event| event["action"] == "admin.user.suspended")
        );
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn catalog_database_and_match_admin_endpoints_are_queryable() {
        let (state, directory) = admin_state();
        let router = build_router(state);
        let (cookie, csrf) = login(router.clone()).await;

        let (status, _, matches) = request(
            router.clone(),
            Method::GET,
            "/api/v1/admin/matches",
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(matches.expect("matches")["schema"], "admin_match_list.v1");

        let (status, _, database) = request(
            router.clone(),
            Method::GET,
            "/api/v1/admin/database",
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(database.expect("database")["tables"].is_array());

        let (status, _, track) = request(
            router,
            Method::POST,
            "/api/v1/admin/music",
            Some(json!({
                "id": "admin-test-track",
                "name": "测试音乐",
                "scene": "lobby",
                "audio_path": "/game/assets/local-game-assets/music/admin-test.mp3",
                "duration_ms": 60000,
                "enabled": true,
                "is_default": false
            })),
            Some(&cookie),
            Some(&csrf),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(track.expect("track")["id"], "admin-test-track");
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
