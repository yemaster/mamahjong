//! Riichi mahjong domain model and rules.

mod config;
mod definition;
mod hand;
mod overrides;
mod preset;
mod progress;
mod snapshot;
mod tile;
mod tile_set;
mod validation;
mod wall;

pub use config::{
    AbortiveDrawRules, BonusRules, DealerContinuation, MatchLength, MatchRules, PlacementUma,
    RiichiRules, RonResolution, ScoringRules, SettlementRules, YakumanValue,
};
pub use definition::{RIICHI_ENGINE_VERSION, RiichiDefinitionError, RiichiRuleDefinition};
pub use hand::{
    Discard, DrawSource, EndReason, HAND_CHECKPOINT_SCHEMA_VERSION, HandCheckpointError, HandError,
    HandEvent, HandInvariantError, HandJudge, HandPhase, HandTransition, KanQuery, Meld, MeldId,
    MeldKind, PlayerHand, Reaction, ReactionKind, RejectAllHandJudge, RiichiHand,
    RiichiHandCheckpoint, RiichiQuery, RiichiStatus, WinQuery, WinSource,
};
pub use overrides::{
    AbortiveDrawRuleOverrides, BonusRuleOverrides, MatchRuleOverrides, PresetRequest,
    RedFiveOverrides, ResolvedRiichiRules, RiichiRuleOverrides, RoomRuleRequest,
    RuleResolutionError, ScoringRuleOverrides, SettlementRuleOverrides,
};
pub use preset::RiichiPreset;
pub use progress::{Honba, ProgressError, RiichiSticks, RoundNumber, Seat, TableProgress, Wind};
pub use snapshot::RiichiRuleSnapshot;
pub use tile::{
    Honor, Rank, Suit, Tile, TileError, TileFace, TileId, TileKind, TileKindIndexError,
};
pub use tile_set::{RedFives, RiichiVariant, TileSet, TileSetError};
pub use validation::{RuleViolation, ValidationErrors};
pub use wall::{SeedGenerationError, Wall, WallError, WallSeed};
