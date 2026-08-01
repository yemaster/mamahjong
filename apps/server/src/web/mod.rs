mod api;
mod session;

use std::path::Path;

use axum::Router;
use axum::http::HeaderValue;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::AppState;

pub use session::AdminSessionError;
pub(crate) use session::{AdminSessionView, AdminSessions};

pub(crate) fn routes(admin_web_dir: &Path, game_web_dir: &Path) -> Router<AppState> {
    let admin_index = admin_web_dir.join("index.html");
    let admin_static = ServeDir::new(admin_web_dir).fallback(ServeFile::new(admin_index));
    let game_index = game_web_dir.join("index.html");
    let game_static = ServeDir::new(game_web_dir).fallback(ServeFile::new(game_index));
    let admin_api = api::routes().layer(SetResponseHeaderLayer::overriding(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    ));
    Router::new()
        .nest("/api/v1/admin", admin_api)
        .nest_service("/admin", admin_static)
        .nest_service("/game", game_static)
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'",
            ),
        ))
}
