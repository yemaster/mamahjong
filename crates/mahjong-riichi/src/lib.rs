//! Riichi mahjong domain model and rules.

mod tile;

pub use tile::{
    Honor, Rank, Suit, Tile, TileError, TileFace, TileId, TileKind, TileKindIndexError,
};
