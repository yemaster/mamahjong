use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::SeatCount;
use serde::{Deserialize, Serialize};

use crate::{Rank, Suit, Tile, TileId, TileKind};

const COPIES_PER_KIND: u8 = 4;
const YONMA_TILE_COUNT: usize = 136;
const SANMA_TILE_COUNT: usize = 108;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiichiVariant {
    Yonma,
    Sanma,
}

impl RiichiVariant {
    #[must_use]
    pub const fn seat_count(self) -> SeatCount {
        match self {
            Self::Yonma => match SeatCount::new(4) {
                Ok(value) => value,
                Err(_) => unreachable!(),
            },
            Self::Sanma => match SeatCount::new(3) {
                Ok(value) => value,
                Err(_) => unreachable!(),
            },
        }
    }

    #[must_use]
    pub const fn tile_count(self) -> usize {
        match self {
            Self::Yonma => YONMA_TILE_COUNT,
            Self::Sanma => SANMA_TILE_COUNT,
        }
    }

    #[must_use]
    pub const fn default_red_fives(self) -> RedFives {
        match self {
            Self::Yonma => RedFives::new_unchecked(1, 1, 1),
            Self::Sanma => RedFives::new_unchecked(0, 1, 1),
        }
    }

    #[must_use]
    pub const fn includes(self, kind: TileKind) -> bool {
        if matches!(self, Self::Yonma) {
            return true;
        }

        match (kind.suit(), kind.rank()) {
            (Some(Suit::Man), Some(rank)) => rank.value() == 1 || rank.value() == 9,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedFives {
    man: u8,
    pin: u8,
    sou: u8,
}

impl RedFives {
    pub const fn new(man: u8, pin: u8, sou: u8) -> Result<Self, TileSetError> {
        if man > COPIES_PER_KIND {
            return Err(TileSetError::TooManyRedFives {
                suit: Suit::Man,
                count: man,
            });
        }
        if pin > COPIES_PER_KIND {
            return Err(TileSetError::TooManyRedFives {
                suit: Suit::Pin,
                count: pin,
            });
        }
        if sou > COPIES_PER_KIND {
            return Err(TileSetError::TooManyRedFives {
                suit: Suit::Sou,
                count: sou,
            });
        }
        Ok(Self::new_unchecked(man, pin, sou))
    }

    const fn new_unchecked(man: u8, pin: u8, sou: u8) -> Self {
        Self { man, pin, sou }
    }

    #[must_use]
    pub const fn for_suit(self, suit: Suit) -> u8 {
        match suit {
            Suit::Man => self.man,
            Suit::Pin => self.pin,
            Suit::Sou => self.sou,
        }
    }

    #[must_use]
    pub const fn total(self) -> u8 {
        self.man + self.pin + self.sou
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileSet {
    variant: RiichiVariant,
    red_fives: RedFives,
    tiles: Box<[Tile]>,
}

impl TileSet {
    pub fn new(variant: RiichiVariant, red_fives: RedFives) -> Result<Self, TileSetError> {
        if matches!(variant, RiichiVariant::Sanma) && red_fives.for_suit(Suit::Man) != 0 {
            return Err(TileSetError::SanmaCannotContainRedManFive);
        }

        let mut tiles = Vec::with_capacity(variant.tile_count());
        for index in 0..TileKind::COUNT as u8 {
            let kind = TileKind::from_index(index)
                .expect("0..TileKind::COUNT always contains valid tile kinds");
            if !variant.includes(kind) {
                continue;
            }

            let red_count = kind.suit().map_or(0, |suit| {
                if kind.rank() == Some(Rank::FIVE) {
                    red_fives.for_suit(suit)
                } else {
                    0
                }
            });

            for copy in 0..COPIES_PER_KIND {
                let id = TileId::new(
                    u16::try_from(tiles.len())
                        .expect("a riichi tile set contains fewer than u16::MAX tiles"),
                );
                let tile = Tile::new(id, kind, copy < red_count)
                    .expect("validated red counts only mark suited fives");
                tiles.push(tile);
            }
        }

        debug_assert_eq!(tiles.len(), variant.tile_count());
        Ok(Self {
            variant,
            red_fives,
            tiles: tiles.into_boxed_slice(),
        })
    }

    pub fn standard(variant: RiichiVariant) -> Self {
        Self::new(variant, variant.default_red_fives())
            .expect("the built-in red-five configuration is valid")
    }

    #[must_use]
    pub const fn variant(&self) -> RiichiVariant {
        self.variant
    }

    #[must_use]
    pub const fn red_fives(&self) -> RedFives {
        self.red_fives
    }

    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    #[must_use]
    pub fn kind_count(&self, kind: TileKind) -> usize {
        self.tiles.iter().filter(|tile| tile.kind() == kind).count()
    }

    #[must_use]
    pub fn red_count(&self, suit: Suit) -> usize {
        self.tiles
            .iter()
            .filter(|tile| tile.is_red() && tile.kind().suit() == Some(suit))
            .count()
    }

    #[must_use]
    pub fn into_tiles(self) -> Vec<Tile> {
        self.tiles.into_vec()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileSetError {
    TooManyRedFives { suit: Suit, count: u8 },
    SanmaCannotContainRedManFive,
}

impl Display for TileSetError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyRedFives { suit, count } => {
                write!(
                    formatter,
                    "red-five count for {suit:?} must be at most 4, got {count}"
                )
            }
            Self::SanmaCannotContainRedManFive => {
                formatter.write_str("sanma removes five-man and cannot contain a red five-man")
            }
        }
    }
}

impl Error for TileSetError {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{Rank, Suit, TileKind};

    use super::{RedFives, RiichiVariant, TileSet, TileSetError};

    #[test]
    fn yonma_standard_set_has_136_tiles_and_three_red_fives() {
        let set = TileSet::standard(RiichiVariant::Yonma);

        assert_eq!(set.len(), 136);
        assert_eq!(set.red_fives().total(), 3);
        assert_eq!(set.red_count(Suit::Man), 1);
        assert_eq!(set.red_count(Suit::Pin), 1);
        assert_eq!(set.red_count(Suit::Sou), 1);

        for index in 0..TileKind::COUNT as u8 {
            let kind = TileKind::from_index(index).expect("valid kind");
            assert_eq!(set.kind_count(kind), 4, "{kind} should have four copies");
        }
    }

    #[test]
    fn sanma_standard_set_removes_two_through_eight_man() {
        let set = TileSet::standard(RiichiVariant::Sanma);

        assert_eq!(set.len(), 108);
        assert_eq!(set.red_fives().total(), 2);
        for rank_value in 1..=9 {
            let kind = TileKind::suited(Suit::Man, Rank::new(rank_value).expect("rank in range"));
            let expected = if rank_value == 1 || rank_value == 9 {
                4
            } else {
                0
            };
            assert_eq!(
                set.kind_count(kind),
                expected,
                "unexpected count for {kind}"
            );
        }
    }

    #[test]
    fn custom_red_fives_replace_normal_fives() {
        let red_fives = RedFives::new(2, 0, 4).expect("valid red configuration");
        let set = TileSet::new(RiichiVariant::Yonma, red_fives).expect("valid tile set");

        assert_eq!(set.red_count(Suit::Man), 2);
        assert_eq!(set.red_count(Suit::Pin), 0);
        assert_eq!(set.red_count(Suit::Sou), 4);
        for suit in [Suit::Man, Suit::Pin, Suit::Sou] {
            assert_eq!(
                set.kind_count(TileKind::suited(suit, Rank::FIVE)),
                4,
                "red tiles still count as the same five kind"
            );
        }
    }

    #[test]
    fn every_physical_tile_id_is_unique() {
        for variant in [RiichiVariant::Yonma, RiichiVariant::Sanma] {
            let set = TileSet::standard(variant);
            let ids: HashSet<_> = set.tiles().iter().map(|tile| tile.id()).collect();

            assert_eq!(ids.len(), set.len());
        }
    }

    #[test]
    fn rejects_more_than_four_red_fives() {
        let error = RedFives::new(0, 5, 0).expect_err("five red copies are impossible");

        assert_eq!(
            error,
            TileSetError::TooManyRedFives {
                suit: Suit::Pin,
                count: 5,
            }
        );
    }

    #[test]
    fn rejects_red_man_five_in_sanma() {
        let red_fives = RedFives::new(1, 1, 1).expect("valid copy counts");
        let error = TileSet::new(RiichiVariant::Sanma, red_fives).expect_err("five-man is removed");

        assert_eq!(error, TileSetError::SanmaCannotContainRedManFive);
    }
}
