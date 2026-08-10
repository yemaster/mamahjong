mod api;
mod archive;
mod audit;
mod clock;
mod config;
mod health;
#[cfg(test)]
mod testing;
mod token;
mod web;

use axum::{Router, routing::get};
use mahjong_core::{MatchId, UserId};
use mamahjong_application::Application;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tower_http::trace::TraceLayer;

pub use archive::{ArchiveError, MatchArchive, MatchRecordSummary, PlayerStatistics};
pub use audit::{AuditDraft, AuditError, AuditEvent, AuditLog};
pub use clock::spawn_sweeper;
pub use config::{ConfigError, ServerConfig};
pub use health::Readiness;
pub use web::AdminSessionError;

#[derive(Clone)]
pub struct AppState {
    readiness: Readiness,
    application: Application,
    archive: MatchArchive,
    audit: AuditLog,
    admin_sessions: web::AdminSessions,
    realtime: api::RealtimeHub,
    ws_tickets: api::WsTickets,
    clock: clock::MonotonicClock,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            readiness: Readiness::new(),
            application: Application::new(),
            archive: MatchArchive::default(),
            audit: AuditLog::memory(),
            admin_sessions: web::AdminSessions::disabled(),
            realtime: api::RealtimeHub::new(),
            ws_tickets: api::WsTickets::new(),
            clock: clock::MonotonicClock::new(),
        }
    }

    pub fn persistent(data_dir: impl AsRef<Path>) -> Result<Self, StateError> {
        Self::persistent_internal(data_dir, None, None)
    }

    pub fn persistent_with_database(
        data_dir: impl AsRef<Path>,
        database_url: &str,
    ) -> Result<Self, StateError> {
        Self::persistent_internal(data_dir, None, Some(database_url))
    }

    pub fn persistent_with_admin(
        data_dir: impl AsRef<Path>,
        cookie_secure: bool,
    ) -> Result<Self, StateError> {
        Self::persistent_internal(data_dir, Some(cookie_secure), None)
    }

    pub fn persistent_with_admin_and_database(
        data_dir: impl AsRef<Path>,
        cookie_secure: bool,
        database_url: &str,
    ) -> Result<Self, StateError> {
        Self::persistent_internal(data_dir, Some(cookie_secure), Some(database_url))
    }

    fn persistent_internal(
        data_dir: impl AsRef<Path>,
        cookie_secure: Option<bool>,
        database_url: Option<&str>,
    ) -> Result<Self, StateError> {
        Ok(Self {
            readiness: Readiness::new(),
            application: match database_url {
                Some(database_url) => {
                    Application::connect_postgres(database_url).map_err(StateError::Application)?
                }
                None => Application::new(),
            },
            archive: MatchArchive::open(&data_dir).map_err(StateError::Archive)?,
            audit: AuditLog::open(data_dir).map_err(StateError::Audit)?,
            admin_sessions: match cookie_secure {
                Some(cookie_secure) => {
                    web::AdminSessions::enabled(cookie_secure).map_err(StateError::AdminSession)?
                }
                None => web::AdminSessions::disabled(),
            },
            realtime: api::RealtimeHub::new(),
            ws_tickets: api::WsTickets::new(),
            clock: clock::MonotonicClock::new(),
        })
    }

    #[must_use]
    pub const fn readiness(&self) -> &Readiness {
        &self.readiness
    }

    #[must_use]
    pub const fn application(&self) -> &Application {
        &self.application
    }

    #[must_use]
    pub const fn audit(&self) -> &AuditLog {
        &self.audit
    }

    pub(crate) const fn admin_sessions(&self) -> &web::AdminSessions {
        &self.admin_sessions
    }

    pub(crate) const fn realtime(&self) -> &api::RealtimeHub {
        &self.realtime
    }

    pub(crate) const fn ws_tickets(&self) -> &api::WsTickets {
        &self.ws_tickets
    }

    /// Milliseconds since this process started; the only time source seat
    /// clocks ever see.
    #[must_use]
    pub(crate) fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    #[cfg(test)]
    pub(crate) fn advance_clock(&self, millis: u64) {
        self.clock.advance(millis);
    }

    pub(crate) async fn record_audit(&self, draft: AuditDraft) -> Result<AuditEvent, AuditError> {
        let audit = self.audit.clone();
        tokio::task::spawn_blocking(move || audit.record(draft))
            .await
            .map_err(|_| AuditError::TaskFailed)?
    }

    pub(crate) async fn persist_match(
        &self,
        actor: &UserId,
        match_id: &MatchId,
    ) -> Result<(), ArchiveError> {
        // 冲击麻将本期不出牌谱，这里静默跳过；不然每走一步都会把「没有牌谱」
        // 当成归档失败，整步操作报 500。
        if !self.application.match_generates_record(match_id) {
            return Ok(());
        }
        let record = self
            .application
            .match_record(actor, match_id)
            .map_err(|_| ArchiveError::TaskFailed)?;
        let archive = self.archive.clone();
        tokio::task::spawn_blocking(move || archive.persist(&record))
            .await
            .map_err(|_| ArchiveError::TaskFailed)?
    }

    pub(crate) async fn player_statistics(
        &self,
        user_id: &UserId,
    ) -> Result<PlayerStatistics, ArchiveError> {
        let archive = self.archive.clone();
        let user_id = user_id.as_str().to_owned();
        tokio::task::spawn_blocking(move || archive.player_statistics(&user_id))
            .await
            .map_err(|_| ArchiveError::TaskFailed)?
    }

    /// 牌谱列表：内存里只留着还没结束的对局，翻历史只能扫归档。
    pub(crate) async fn player_records(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<MatchRecordSummary>, ArchiveError> {
        let archive = self.archive.clone();
        let user_id = user_id.as_str().to_owned();
        tokio::task::spawn_blocking(move || archive.player_records(&user_id))
            .await
            .map_err(|_| ArchiveError::TaskFailed)?
    }

    /// 归档里的一份牌谱，服务端重启之后重演就靠它。
    pub(crate) async fn archived_record(
        &self,
        match_id: &MatchId,
        user_id: &UserId,
    ) -> Result<Option<serde_json::Value>, ArchiveError> {
        let archive = self.archive.clone();
        let match_id = match_id.as_str().to_owned();
        let user_id = user_id.as_str().to_owned();
        tokio::task::spawn_blocking(move || archive.record(&match_id, &user_id))
            .await
            .map_err(|_| ArchiveError::TaskFailed)?
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_router(state: AppState) -> Router {
    build_router_with_web(
        state,
        PathBuf::from("apps/admin-web/dist"),
        PathBuf::from("apps/game-web/dist"),
    )
}

pub fn build_router_with_admin_web(state: AppState, web_dir: impl AsRef<Path>) -> Router {
    build_router_with_web(state, web_dir, PathBuf::from("apps/game-web/dist"))
}

pub fn build_router_with_web(
    state: AppState,
    admin_web_dir: impl AsRef<Path>,
    game_web_dir: impl AsRef<Path>,
) -> Router {
    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .merge(api::routes())
        .merge(web::routes(admin_web_dir.as_ref(), game_web_dir.as_ref()))
        .with_state(state)
        .layer(TraceLayer::new_for_http().on_response(
            |response: &axum::http::Response<_>, latency: Duration, _span: &tracing::Span| {
                let status = response.status();
                let latency_ms = latency.as_secs_f64() * 1_000.0;
                if status.is_server_error() {
                    tracing::error!(%status, latency_ms, "HTTP 请求完成");
                } else if status.is_client_error() {
                    tracing::warn!(%status, latency_ms, "HTTP 请求完成");
                } else {
                    tracing::debug!(%status, latency_ms, "HTTP 请求完成");
                }
            },
        ))
}

#[derive(Debug)]
pub enum StateError {
    Application(mamahjong_application::ApplicationError),
    Archive(ArchiveError),
    Audit(AuditError),
    AdminSession(web::AdminSessionError),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Application(error) => error.fmt(formatter),
            Self::Archive(error) => error.fmt(formatter),
            Self::Audit(error) => error.fmt(formatter),
            Self::AdminSession(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for StateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Application(error) => Some(error),
            Self::Archive(error) => Some(error),
            Self::Audit(error) => Some(error),
            Self::AdminSession(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::{AppState, build_router};

    #[tokio::test]
    async fn liveness_does_not_depend_on_readiness() {
        let response = build_router(AppState::new())
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readiness_reflects_application_state() {
        let state = AppState::new();
        let router = build_router(state.clone());

        let unavailable = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);

        state.readiness().mark_ready();
        let available = router
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(available.status(), StatusCode::OK);
    }
}
