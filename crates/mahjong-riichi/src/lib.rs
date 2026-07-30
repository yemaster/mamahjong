//! Riichi mahjong domain model and rules.

mod tile;
mod tile_set;

pub use tile::{
    Honor, Rank, Suit, Tile, TileError, TileFace, TileId, TileKind, TileKindIndexError,
};
pub use tile_set::{RedFives, RiichiVariant, TileSet, TileSetError};
