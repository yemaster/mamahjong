//! 四川麻将（血战到底）的领域模型与规则引擎。
//!
//! 与 `mahjong-riichi` / `mahjong-impact` 完全独立：只用万筒索三门、无字牌无财神，
//! 每家从 0 分打起，杠（雨）与胡都即时结算。一家胡后盖牌退出、其余继续（血战到底），
//! 三家胡或牌山摸尽本局结束；流局查花猪、查大叫。

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

pub use config::{MatchRules, SEAT_COUNT, SichuanRules, ThinkingTimeRules};
pub use definition::{
    SICHUAN_ENGINE_VERSION, SICHUAN_RULE_SET_ID, SichuanDefinitionError, SichuanRuleDefinition,
};
pub use hand::{
    Discard, DrawSource, EndReason, HandError, HandPhase, KanEvent, Meld, MeldId, MeldKind,
    PlayerHand, Reaction, ReactionKind, ReactionOptions, SichuanHand, TurnAction, TurnActions,
    WinnerRecord,
};
pub use match_state::{
    FLOWER_PIG_POINTS, HandSettlement, MatchError, NOTEN_POINTS, PlayerResult, QueSettlement,
    SichuanMatch,
};
pub use overrides::{
    MatchRuleOverrides, PresetRequest, ResolvedSichuanRules, RuleResolutionError,
    SichuanRoomRuleRequest, SichuanRuleOverrides,
};
pub use preset::SichuanPreset;
pub use progress::{ProgressError, Seat, TableProgress};
pub use scoring::{
    HandShapes, MAX_FAN, MeldSummary, WinContext, WinEvaluation, Yaku, YakuValue, evaluate,
    score_for,
};
pub use snapshot::SichuanRuleSnapshot;
pub use tile::{Honor, Rank, Suit, Tile, TileError, TileId, TileKind, full_tile_set};
pub use validation::{RuleViolation, ValidationErrors};
pub use wall::{
    Dice, ExchangeDirection, STACKS_BY_SEAT, SeedGenerationError, TOTAL_STACKS, WALL_TILE_COUNT,
    Wall, WallError, WallSeed,
};
