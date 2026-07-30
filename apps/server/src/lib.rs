mod api;
mod config;
mod health;

use axum::{Router, routing::get};
use mamahjong_application::Application;

pub use config::{ConfigError, ServerConfig};
pub use health::Readiness;

#[derive(Clone)]
pub struct AppState {
    readiness: Readiness,
    application: Application,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            readiness: Readiness::new(),
            application: Application::new(),
        }
    }

    #[must_use]
    pub const fn readiness(&self) -> &Readiness {
        &self.readiness
    }

    #[must_use]
    pub const fn application(&self) -> &Application {
        &self.application
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .merge(api::routes())
        .with_state(state)
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
