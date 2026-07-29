use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::AppState;

#[derive(Clone, Debug)]
pub struct Readiness {
    ready: Arc<AtomicBool>,
}

impl Readiness {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    pub fn mark_not_ready(&self) {
        self.ready.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

impl Default for Readiness {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

pub(crate) async fn live() -> impl IntoResponse {
    Json(HealthResponse::healthy())
}

pub(crate) async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    if state.readiness().is_ready() {
        (StatusCode::OK, Json(HealthResponse::healthy()))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse::not_ready()),
        )
    }
}

impl HealthResponse {
    const fn healthy() -> Self {
        Self {
            status: "ok",
            service: "mamahjong-server",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    const fn not_ready() -> Self {
        Self {
            status: "not_ready",
            service: "mamahjong-server",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}
