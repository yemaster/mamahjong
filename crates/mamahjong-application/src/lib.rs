//! Application services for identity, rooms, and match orchestration.

mod error;
mod game;
mod identity;
mod record;
mod room;
mod service;
mod store;

pub use error::{ApplicationError, ErrorCode};
pub use game::{
    GameCommand, GameEventRecord, MatchPlayer, ObserverMatch, ObserverPlayer, SubmitGameCommand,
};
pub use identity::{
    AccountStatus, CharacterSummary, Nickname, RankSummary, Session, TitleSummary, User,
    UserProfile,
};
pub use record::MatchRecord;
pub use room::{GameRuleSnapshot, Room, RoomLifecycle, RoomMember, RoomVisibility};
pub use service::{
    Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdateProfile, UpdateRoom,
};
