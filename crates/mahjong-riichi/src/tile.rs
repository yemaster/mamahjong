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

    const fn code(self) -> char {
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
    pub const FIVE: Self = Self(5);

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

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        self.0 == Self::MIN || self.0 == Self::MAX
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

    pub const fn from_index(index: u8) -> Result<Self, TileKindIndexError> {
        if index < Self::COUNT as u8 {
            Ok(Self(index))
        } else {
            Err(TileKindIndexError)
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub const fn suit(self) -> Option<Suit> {
        match self.0 / 9 {
            0 if self.0 < Self::SUITED_KIND_COUNT => Some(Suit::Man),
            1 => Some(Suit::Pin),
            2 => Some(Suit::Sou),
            _ => None,
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

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        match self.rank() {
            Some(rank) => rank.is_terminal(),
            None => false,
        }
    }

    #[must_use]
    pub const fn is_terminal_or_honor(self) -> bool {
        self.is_honor() || self.is_terminal()
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
        let face = TileFace::from_str(value)?;
        if face.is_red() {
            return Err(TileError::RedCodeIsNotTileKind);
        }
        Ok(face.kind())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileKindIndexError;

impl Display for TileKindIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("tile kind index must be between 0 and 33")
    }
}

impl Error for TileKindIndexError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileFace {
    kind: TileKind,
    red: bool,
}

impl TileFace {
    pub const fn new(kind: TileKind, red: bool) -> Result<Self, TileError> {
        if red && !matches!(kind.rank(), Some(Rank::FIVE)) {
            return Err(TileError::RedOnlyFive);
        }
        Ok(Self { kind, red })
    }

    #[must_use]
    pub const fn kind(self) -> TileKind {
        self.kind
    }

    #[must_use]
    pub const fn is_red(self) -> bool {
        self.red
    }
}

impl Display for TileFace {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        if self.red {
            let Some(suit) = self.kind.suit() else {
                return Err(fmt::Error);
            };
            write!(formatter, "0{}", suit.code())
        } else {
            Display::fmt(&self.kind, formatter)
        }
    }
}

impl FromStr for TileFace {
    type Err = TileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let [number, family] = value.as_bytes() else {
            return Err(TileError::InvalidCode);
        };

        if let Some(suit) = Suit::from_code(*family) {
            let (rank, red) = if *number == b'0' {
                (Rank::FIVE, true)
            } else {
                let value = number.checked_sub(b'0').ok_or(TileError::InvalidCode)?;
                (Rank::new(value)?, false)
            };
            return Self::new(TileKind::suited(suit, rank), red);
        }

        if *family == b'z' {
            let index = number.checked_sub(b'1').ok_or(TileError::InvalidCode)?;
            let honor = Honor::from_index(index).ok_or(TileError::InvalidCode)?;
            return Self::new(TileKind::honor(honor), false);
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
    face: TileFace,
}

impl Tile {
    pub const fn new(id: TileId, kind: TileKind, red: bool) -> Result<Self, TileError> {
        let face = match TileFace::new(kind, red) {
            Ok(face) => face,
            Err(error) => return Err(error),
        };
        Ok(Self { id, face })
    }

    #[must_use]
    pub const fn id(self) -> TileId {
        self.id
    }

    #[must_use]
    pub const fn face(self) -> TileFace {
        self.face
    }

    #[must_use]
    pub const fn kind(self) -> TileKind {
        self.face.kind()
    }

    #[must_use]
    pub const fn is_red(self) -> bool {
        self.face.is_red()
    }
}

impl Display for Tile {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.face, formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileError {
    InvalidRank,
    InvalidCode,
    RedOnlyFive,
    RedCodeIsNotTileKind,
}

impl Display for TileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRank => "numbered tile rank must be between 1 and 9",
            Self::InvalidCode => "invalid tile code",
            Self::RedOnlyFive => "only a suited five can be red",
            Self::RedCodeIsNotTileKind => "a red tile code describes a tile face, not a tile kind",
        };
        formatter.write_str(message)
    }
}

impl Error for TileError {}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{Honor, Rank, Suit, Tile, TileFace, TileId, TileKind};

    #[test]
    fn all_kinds_round_trip_through_index_and_code() {
        for index in 0..TileKind::COUNT as u8 {
            let kind = TileKind::from_index(index).expect("valid index");
            let reparsed: TileKind = kind.to_string().parse().expect("valid code");

            assert_eq!(kind.index(), index as usize);
            assert_eq!(reparsed, kind);
        }
    }

    #[test]
    fn suited_kind_exposes_suit_and_rank() {
        let kind = TileKind::suited(Suit::Pin, Rank::new(7).expect("valid rank"));

        assert_eq!(kind.suit(), Some(Suit::Pin));
        assert_eq!(kind.rank().map(Rank::value), Some(7));
        assert!(!kind.is_honor());
    }

    #[test]
    fn honor_kind_has_no_suit_or_rank() {
        let kind = TileKind::honor(Honor::Green);

        assert_eq!(kind.to_string(), "6z");
        assert_eq!(kind.honor_value(), Some(Honor::Green));
        assert_eq!(kind.suit(), None);
        assert_eq!(kind.rank(), None);
        assert!(kind.is_honor());
    }

    #[test]
    fn parses_red_five_as_tile_face() {
        let face: TileFace = "0s".parse().expect("valid red five");

        assert!(face.is_red());
        assert_eq!(face.kind().suit(), Some(Suit::Sou));
        assert_eq!(face.kind().rank(), Some(Rank::FIVE));
        assert_eq!(face.to_string(), "0s");
    }

    #[test]
    fn rejects_invalid_codes() {
        for code in ["", "10m", "0z", "8z", "5x", "０m", "M5"] {
            assert!(
                code.parse::<TileFace>().is_err(),
                "{code} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_red_non_five() {
        let result = TileFace::new(
            TileKind::suited(Suit::Man, Rank::new(4).expect("valid rank")),
            true,
        );

        assert!(result.is_err());
    }

    #[test]
    fn tile_keeps_physical_identity_separate_from_face() {
        let kind = TileKind::suited(Suit::Man, Rank::FIVE);
        let tile = Tile::new(TileId::new(42), kind, true).expect("valid red tile");

        assert_eq!(tile.id().value(), 42);
        assert_eq!(tile.kind(), kind);
        assert!(tile.is_red());
        assert_eq!(tile.to_string(), "0m");
    }

    #[test]
    fn hot_path_types_remain_compact() {
        assert_eq!(size_of::<TileKind>(), 1);
        assert_eq!(size_of::<Tile>(), 4);
    }
}
