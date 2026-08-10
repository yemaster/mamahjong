//! 冲击麻将（Impact Mahjong）的领域模型与规则引擎。
//!
//! 与 `mahjong-riichi` 完全独立：不吃、只自摸、财神百搭，点数之外另有一本「杠点」账，
//! 并且有一整套「全交」牌型。两套引擎只共用 `mahjong-core` 里的规则元数据。

mod config;
mod definition;
mod hand;
mod match_state;
mod overrides;
mod preset;
mod progress;
mod scoring;
mod snapshot;
mod tile;
mod validation;
mod wall;

pub use config::{
    ALL_IN_WINNER_POINTS, AllInRules, BASE_VALUE, DEALER_STREAK_VALUE, INITIAL_KAN_POINTS,
    INITIAL_POINTS, ImpactMode, ImpactRules, KanRules, MatchRules, SEAT_COUNT, SpecialRules,
    ThinkingTimeRules,
};
pub use definition::{
    IMPACT_ENGINE_VERSION, IMPACT_RULE_SET_ID, ImpactDefinitionError, ImpactRuleDefinition,
};
pub use hand::{
    Discard, DrawSource, EndReason, HandError, HandOutcome, HandPhase, ImpactHand, Meld, MeldId,
    MeldKind, PlayerHand, Reaction, ReactionKind, ReactionOptions, TurnAction, TurnActions,
};
pub use match_state::{HandSettlement, ImpactMatch, MatchError, PlayerResult};
pub use overrides::{
    AllInRuleOverrides, ImpactRoomRuleRequest, ImpactRuleOverrides, KanRuleOverrides,
    MatchRuleOverrides, PresetRequest, ResolvedImpactRules, RuleResolutionError,
    SpecialRuleOverrides,
};
pub use preset::ImpactPreset;
pub use progress::{DealerStreak, ProgressError, Seat, TableProgress};
pub use scoring::{
    AllInKind, HandShapes, MeldSummary, WinContext, WinEvaluation, Yaku, YakuValue, evaluate,
};
pub use snapshot::ImpactRuleSnapshot;
pub use tile::{Honor, Rank, Suit, Tile, TileError, TileId, TileKind, full_tile_set, joker_of};
pub use validation::{RuleViolation, ValidationErrors};
pub use wall::{
    Dice, STACKS_PER_WALL, SeedGenerationError, TILES_PER_STACK, WALL_COUNT, WALL_TILE_COUNT, Wall,
    WallError, WallSeed,
};
