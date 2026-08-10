//! 冲击麻将的牌面。
//!
//! 编码和 `mahjong-riichi` 完全一致（`1m`..`9s`、`1z`..`7z`），前端的牌面资源表因此
//! 可以直接复用。区别只有一条：冲击麻将没有赤牌，所以这里不存在 `TileFace` 那一层。

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Suit {
    Man = 0,
    Pin = 1,
    Sou = 2,
}

impl Suit {
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn code(self) -> char {
        match self {
            Self::Man => 'm',
            Self::Pin => 'p',
            Self::Sou => 's',
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            b'm' => Some(Self::Man),
            b'p' => Some(Self::Pin),
            b's' => Some(Self::Sou),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Rank(u8);

impl Rank {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 9;

    pub const fn new(value: u8) -> Result<Self, TileError> {
        if value >= Self::MIN && value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(TileError::InvalidRank)
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Honor {
    East = 0,
    South = 1,
    West = 2,
    North = 3,
    White = 4,
    Green = 5,
    Red = 6,
}

impl Honor {
    pub const ALL: [Self; 7] = [
        Self::East,
        Self::South,
        Self::West,
        Self::North,
        Self::White,
        Self::Green,
        Self::Red,
    ];

    const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::East),
            1 => Some(Self::South),
            2 => Some(Self::West),
            3 => Some(Self::North),
            4 => Some(Self::White),
            5 => Some(Self::Green),
            6 => Some(Self::Red),
            _ => None,
        }
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8 + 1
    }

    #[must_use]
    pub const fn is_wind(self) -> bool {
        matches!(self, Self::East | Self::South | Self::West | Self::North)
    }

    /// 财神轮转的下一张：风牌走 东→南→西→北→东，三元牌走 中→发→白→中。
    ///
    /// 三元牌那一圈必须显式写死：`Honor` 的索引序是 白(5z)/发(6z)/中(7z)，
    /// 而规则要求的顺序是 中→发→白，按索引加一会反过来。
    #[must_use]
    pub const fn next_in_cycle(self) -> Self {
        match self {
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
            Self::Red => Self::Green,
            Self::Green => Self::White,
            Self::White => Self::Red,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TileKind(u8);

impl TileKind {
    pub const COUNT: usize = 34;
    const SUITED_KIND_COUNT: u8 = 27;

    #[must_use]
    pub const fn suited(suit: Suit, rank: Rank) -> Self {
        Self(suit as u8 * 9 + rank.value() - 1)
    }

    #[must_use]
    pub const fn honor(honor: Honor) -> Self {
        Self(Self::SUITED_KIND_COUNT + honor as u8)
    }

    pub const fn from_index(index: u8) -> Result<Self, TileError> {
        if index < Self::COUNT as u8 {
            Ok(Self(index))
        } else {
            Err(TileError::InvalidKindIndex)
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub const fn suit(self) -> Option<Suit> {
        if self.0 >= Self::SUITED_KIND_COUNT {
            return None;
        }
        match self.0 / 9 {
            0 => Some(Suit::Man),
            1 => Some(Suit::Pin),
            _ => Some(Suit::Sou),
        }
    }

    #[must_use]
    pub const fn rank(self) -> Option<Rank> {
        if self.0 < Self::SUITED_KIND_COUNT {
            Some(Rank(self.0 % 9 + 1))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn honor_value(self) -> Option<Honor> {
        if self.0 < Self::SUITED_KIND_COUNT {
            None
        } else {
            Honor::from_index(self.0 - Self::SUITED_KIND_COUNT)
        }
    }

    #[must_use]
    pub const fn is_honor(self) -> bool {
        self.0 >= Self::SUITED_KIND_COUNT
    }
}

/// 由财神指示牌推出财神。
///
/// 数牌在同一花色里循环 1→2→…→9→1；风牌 东→南→西→北→东；
/// 三元牌 中→发→白→中。
#[must_use]
pub const fn joker_of(indicator: TileKind) -> TileKind {
    match (indicator.suit(), indicator.rank()) {
        (Some(suit), Some(rank)) => {
            let next = if rank.value() == Rank::MAX {
                Rank::MIN
            } else {
                rank.value() + 1
            };
            TileKind::suited(suit, Rank(next))
        }
        _ => match indicator.honor_value() {
            Some(honor) => TileKind::honor(honor.next_in_cycle()),
            // `TileKind` 只有数牌和字牌两种，这一支不可达。
            None => indicator,
        },
    }
}

impl Display for TileKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        if let (Some(suit), Some(rank)) = (self.suit(), self.rank()) {
            write!(formatter, "{}{}", rank.value(), suit.code())
        } else if let Some(honor) = self.honor_value() {
            write!(formatter, "{}z", honor.code())
        } else {
            Err(fmt::Error)
        }
    }
}

impl FromStr for TileKind {
    type Err = TileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let [number, family] = value.as_bytes() else {
            return Err(TileError::InvalidCode);
        };

        if let Some(suit) = Suit::from_code(*family) {
            let rank = number.checked_sub(b'0').ok_or(TileError::InvalidCode)?;
            return Ok(Self::suited(suit, Rank::new(rank)?));
        }

        if *family == b'z' {
            let index = number.checked_sub(b'1').ok_or(TileError::InvalidCode)?;
            let honor = Honor::from_index(index).ok_or(TileError::InvalidCode)?;
            return Ok(Self::honor(honor));
        }

        Err(TileError::InvalidCode)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileId(u16);

impl TileId {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tile {
    id: TileId,
    kind: TileKind,
}

impl Tile {
    #[must_use]
    pub const fn new(id: TileId, kind: TileKind) -> Self {
        Self { id, kind }
    }

    #[must_use]
    pub const fn id(self) -> TileId {
        self.id
    }

    #[must_use]
    pub const fn kind(self) -> TileKind {
        self.kind
    }
}

impl Display for Tile {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.kind, formatter)
    }
}

/// 一副牌：34 种 × 4 张，按牌种顺序排好，`TileId` 就是下标。
#[must_use]
pub fn full_tile_set() -> Vec<Tile> {
    let mut tiles = Vec::with_capacity(TileKind::COUNT * 4);
    for index in 0..TileKind::COUNT {
        let kind = TileKind::from_index(u8::try_from(index).expect("kind index fits u8"))
            .expect("kind index is in range");
        for copy in 0..4_u16 {
            let id = TileId::new(u16::try_from(index).expect("kind index fits u16") * 4 + copy);
            tiles.push(Tile::new(id, kind));
        }
    }
    tiles
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileError {
    InvalidRank,
    InvalidCode,
    InvalidKindIndex,
}

impl Display for TileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRank => "tile rank must be between 1 and 9",
            Self::InvalidCode => "tile code must look like 1m, 9s or 7z",
            Self::InvalidKindIndex => "tile kind index must be between 0 and 33",
        };
        formatter.write_str(message)
    }
}

impl Error for TileError {}

#[cfg(test)]
mod tests {
    use super::{Honor, Rank, Suit, Tile, TileKind, full_tile_set, joker_of};

    fn kind(code: &str) -> TileKind {
        code.parse().expect("valid tile code")
    }

    #[test]
    fn number_joker_wraps_from_nine_to_one() {
        assert_eq!(joker_of(kind("8p")), kind("9p"));
        assert_eq!(joker_of(kind("9p")), kind("1p"));
        assert_eq!(joker_of(kind("9m")), kind("1m"));
        assert_eq!(joker_of(kind("9s")), kind("1s"));
    }

    #[test]
    fn wind_joker_wraps_from_north_to_east() {
        assert_eq!(joker_of(kind("1z")), kind("2z"));
        assert_eq!(joker_of(kind("2z")), kind("3z"));
        assert_eq!(joker_of(kind("3z")), kind("4z"));
        assert_eq!(joker_of(kind("4z")), kind("1z"));
    }

    #[test]
    fn dragon_joker_runs_red_green_white() {
        // 中(7z) → 发(6z) → 白(5z) → 中(7z)
        assert_eq!(joker_of(kind("7z")), kind("6z"));
        assert_eq!(joker_of(kind("6z")), kind("5z"));
        assert_eq!(joker_of(kind("5z")), kind("7z"));
    }

    #[test]
    fn joker_cycle_visits_every_kind_exactly_once() {
        for start in 0..TileKind::COUNT {
            let start = TileKind::from_index(u8::try_from(start).expect("fits u8"))
                .expect("valid kind index");
            let mut current = start;
            let cycle_length = if start.is_honor() {
                if start.honor_value().expect("honor").is_wind() {
                    4
                } else {
                    3
                }
            } else {
                9
            };
            for _ in 0..cycle_length {
                current = joker_of(current);
            }
            assert_eq!(current, start, "cycle from {start} did not close");
        }
    }

    #[test]
    fn tile_codes_round_trip() {
        for tile in full_tile_set() {
            let code = tile.to_string();
            assert_eq!(code.parse::<TileKind>().expect("valid code"), tile.kind());
        }
    }

    #[test]
    fn full_tile_set_holds_four_of_each_kind() {
        let tiles = full_tile_set();
        assert_eq!(tiles.len(), 136);
        let mut counts = [0_u8; TileKind::COUNT];
        for tile in &tiles {
            counts[tile.kind().index()] += 1;
        }
        assert!(counts.iter().all(|count| *count == 4));
    }

    #[test]
    fn tile_ids_are_unique() {
        let tiles: Vec<Tile> = full_tile_set();
        let mut ids: Vec<u16> = tiles.iter().map(|tile| tile.id().value()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), tiles.len());
    }

    #[test]
    fn suited_kinds_expose_suit_and_rank() {
        let five_pin = TileKind::suited(Suit::Pin, Rank::new(5).expect("valid rank"));
        assert_eq!(five_pin.suit(), Some(Suit::Pin));
        assert_eq!(five_pin.rank().map(Rank::value), Some(5));
        assert!(!five_pin.is_honor());

        let east = TileKind::honor(Honor::East);
        assert!(east.is_honor());
        assert_eq!(east.suit(), None);
        assert_eq!(east.honor_value(), Some(Honor::East));
    }
}
