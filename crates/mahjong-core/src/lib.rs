//! Network- and storage-independent mahjong domain.

pub mod identifier;
pub mod rules;
pub mod version;

pub use identifier::{
    CommandId, ConnectionId, IdParseError, MatchId, RoomId, SessionId, TicketId, UserId,
};
pub use rules::{
    EngineVersion, EngineVersionParseError, PresetId, PresetRef, RuleConfig, RuleDefinition,
    RuleDescriptor, RuleMetadataError, RuleSetId, RuleSnapshot, SeatCount,
};
pub use version::{AggregateVersion, VersionOverflow};
