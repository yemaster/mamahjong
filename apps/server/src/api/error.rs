use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use mamahjong_application::{ApplicationError, ErrorCode};
use serde::Serialize;

#[derive(Debug)]
pub(super) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retryable: bool,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    kind: &'static str,
    schema: &'static str,
    code: &'static str,
    message: String,
    retryable: bool,
}

impl ApiError {
    pub(super) fn invalid_json(_rejection: JsonRejection) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "request.invalid_json",
            message: "request body is not valid for this endpoint".to_owned(),
            retryable: false,
        }
    }

    pub(super) fn invalid_id() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "request.invalid_id",
            message: "resource ID is invalid".to_owned(),
            retryable: false,
        }
    }

    pub(super) fn invalid_rule_set() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "request.invalid_rule_set",
            message: "rule_set_id is not supported for matchmaking".to_owned(),
            retryable: false,
        }
    }

    pub(super) fn missing_bearer() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "auth.missing_bearer",
            message: "a bearer session token is required".to_owned(),
            retryable: false,
        }
    }

    pub(super) fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "server.internal",
            message: "internal server error".to_owned(),
            retryable: true,
        }
    }
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        let code = error.code();
        let status = match code {
            ErrorCode::InvalidLoginName
            | ErrorCode::InvalidPassword
            | ErrorCode::InvalidNickname
            | ErrorCode::InvalidRoomName
            | ErrorCode::InvalidRuleConfiguration => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCredentials | ErrorCode::InvalidSession => StatusCode::UNAUTHORIZED,
            ErrorCode::UserUnavailable | ErrorCode::NotRoomOwner | ErrorCode::NotMatchPlayer => {
                StatusCode::FORBIDDEN
            }
            ErrorCode::RoomNotFound | ErrorCode::MatchNotFound => StatusCode::NOT_FOUND,
            ErrorCode::LoginNameTaken
            | ErrorCode::RoomClosed
            | ErrorCode::RoomFull
            | ErrorCode::AlreadyRoomMember
            | ErrorCode::NotRoomMember
            | ErrorCode::RoomVersionConflict
            | ErrorCode::RoomPlaying
            | ErrorCode::RoomNotReady
            | ErrorCode::MatchVersionConflict
            | ErrorCode::MatchFinished
            | ErrorCode::AlreadyQueued
            | ErrorCode::MatchmakingTicketNotWaiting => StatusCode::CONFLICT,
            ErrorCode::UserBusy => StatusCode::CONFLICT,
            ErrorCode::MatchmakingTicketNotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidGameCommand => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            code: code.as_str(),
            message: error.to_string(),
            retryable: matches!(
                code,
                ErrorCode::RoomVersionConflict
                    | ErrorCode::MatchVersionConflict
                    | ErrorCode::Internal
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                kind: "error",
                schema: "error.v1",
                code: self.code,
                message: self.message,
                retryable: self.retryable,
            }),
        )
            .into_response()
    }
}
