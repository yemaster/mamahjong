//! Application services for identity, rooms, and match orchestration.

mod error;
mod game;
mod identity;
mod matchmaking;
mod record;
mod room;
mod service;
mod store;

pub use error::{ApplicationError, ErrorCode};
pub use game::{
    AddedKanOption, GameCommand, GameEventRecord, MatchPlayer, ObserverMatch, ObserverPlayer,
    SubmitGameCommand, TurnActions,
};
pub use identity::{
    AccountStatus, CharacterSummary, Nickname, RankSummary, Session, TitleSummary, User,
    UserProfile,
};
pub use matchmaking::{MatchmakingStatus, MatchmakingTicket};
pub use record::MatchRecord;
pub use room::{GameRuleSnapshot, Room, RoomLifecycle, RoomMember, RoomVisibility};
pub use service::{
    Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdateProfile, UpdateRoom,
};
