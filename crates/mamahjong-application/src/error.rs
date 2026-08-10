use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidLoginName,
    InvalidPassword,
    InvalidNickname,
    InvalidCharacter,
    CharacterNotFound,
    CharacterDefaultRequired,
    InvalidTablecloth,
    TableclothNotFound,
    TableclothDefaultRequired,
    InvalidMusicTrack,
    MusicTrackNotFound,
    MusicTrackDefaultRequired,
    LoginNameTaken,
    InvalidCredentials,
    InvalidSession,
    UserUnavailable,
    InvalidRoomName,
    InvalidRuleConfiguration,
    RoomNotFound,
    RoomClosed,
    RoomFull,
    AlreadyRoomMember,
    NotRoomMember,
    NotRoomOwner,
    RoomVersionConflict,
    RoomPlaying,
    RoomNotReady,
    MatchNotFound,
    NotMatchPlayer,
    MatchVersionConflict,
    InvalidGameCommand,
    MatchFinished,
    AlreadyQueued,
    MatchmakingTicketNotFound,
    MatchmakingTicketNotWaiting,
    UserBusy,
    Internal,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidLoginName => "request.invalid_login_name",
            Self::InvalidPassword => "request.invalid_password",
            Self::InvalidNickname => "request.invalid_nickname",
            Self::InvalidCharacter => "request.invalid_character",
            Self::CharacterNotFound => "character.not_found",
            Self::CharacterDefaultRequired => "character.default_required",
            Self::InvalidTablecloth => "request.invalid_tablecloth",
            Self::TableclothNotFound => "tablecloth.not_found",
            Self::TableclothDefaultRequired => "tablecloth.default_required",
            Self::InvalidMusicTrack => "request.invalid_music_track",
            Self::MusicTrackNotFound => "music_track.not_found",
            Self::MusicTrackDefaultRequired => "music_track.default_required",
            Self::LoginNameTaken => "auth.login_name_taken",
            Self::InvalidCredentials => "auth.invalid_credentials",
            Self::InvalidSession => "auth.invalid_session",
            Self::UserUnavailable => "auth.user_unavailable",
            Self::InvalidRoomName => "request.invalid_room_name",
            Self::InvalidRuleConfiguration => "request.invalid_rule_config",
            Self::RoomNotFound => "room.not_found",
            Self::RoomClosed => "room.closed",
            Self::RoomFull => "room.full",
            Self::AlreadyRoomMember => "room.already_member",
            Self::NotRoomMember => "room.not_member",
            Self::NotRoomOwner => "room.not_owner",
            Self::RoomVersionConflict => "room.version_conflict",
            Self::RoomPlaying => "room.playing",
            Self::RoomNotReady => "room.not_ready",
            Self::MatchNotFound => "game.not_found",
            Self::NotMatchPlayer => "game.not_player",
            Self::MatchVersionConflict => "game.stale_version",
            Self::InvalidGameCommand => "game.invalid_command",
            Self::MatchFinished => "game.finished",
            Self::AlreadyQueued => "matchmaking.already_queued",
            Self::MatchmakingTicketNotFound => "matchmaking.ticket_not_found",
            Self::MatchmakingTicketNotWaiting => "matchmaking.ticket_not_waiting",
            Self::UserBusy => "lobby.user_busy",
            Self::Internal => "server.internal",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationError {
    code: ErrorCode,
    message: String,
}

impl ApplicationError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }
}

impl Display for ApplicationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ApplicationError {}
