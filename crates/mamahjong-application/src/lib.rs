//! Application services for identity, rooms, and match orchestration.

mod error;
mod identity;
mod room;
mod service;
mod store;

pub use error::{ApplicationError, ErrorCode};
pub use identity::{
    AccountStatus, CharacterSummary, Nickname, RankSummary, Session, TitleSummary, User,
    UserProfile,
};
pub use room::{GameRuleSnapshot, Room, RoomLifecycle, RoomMember, RoomVisibility};
pub use service::{
    Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdateProfile, UpdateRoom,
};
