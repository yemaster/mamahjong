use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use mamahjong_application::User;

use super::error::ApiError;
use crate::AppState;

pub(super) struct AuthenticatedUser(User);

impl AuthenticatedUser {
    pub(super) const fn user(&self) -> &User {
        &self.0
    }
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(ApiError::missing_bearer)?;
        let token = authorization
            .strip_prefix("Bearer ")
            .filter(|token| !token.is_empty())
            .ok_or_else(ApiError::missing_bearer)?;
        state
            .application()
            .authenticate(token)
            .map(Self)
            .map_err(ApiError::from)
    }
}
