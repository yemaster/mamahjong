use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

use crate::{RiichiVariant, Tile, TileSet};

const WALL_SEED_SIZE: usize = 32;
const DEAD_WALL_SIZE: usize = 14;
const MAX_RINSHAN_DRAWS: u8 = 4;
const MAX_DORA_INDICATORS: u8 = 5;
const RINSHAN_OFFSETS: [usize; MAX_RINSHAN_DRAWS as usize] = [13, 12, 11, 10];
const DORA_OFFSETS: [usize; MAX_DORA_INDICATORS as usize] = [8, 6, 4, 2, 0];
const URA_DORA_OFFSETS: [usize; MAX_DORA_INDICATORS as usize] = [9, 7, 5, 3, 1];

#[derive(Clone, Eq, PartialEq)]
pub struct WallSeed([u8; WALL_SEED_SIZE]);

impl WallSeed {
    pub fn generate() -> Result<Self, SeedGenerationError> {
        let mut bytes = [0_u8; WALL_SEED_SIZE];
        getrandom::fill(&mut bytes).map_err(SeedGenerationError)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; WALL_SEED_SIZE]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn expose_bytes(&self) -> &[u8; WALL_SEED_SIZE] {
        &self.0
    }
}

impl Debug for WallSeed {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("WallSeed([REDACTED])")
    }
}

#[derive(Debug)]
pub struct SeedGenerationError(getrandom::Error);

impl Display for SeedGenerationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to obtain operating-system randomness: {}",
            self.0
        )
    }
}

impl Error for SeedGenerationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wall {
    variant: RiichiVariant,
    tiles: Box<[Tile]>,
    live_draw_index: usize,
    live_end: usize,
    rinshan_draws: u8,
    revealed_dora_count: u8,
}

impl Wall {
    #[must_use]
    pub fn new(tile_set: TileSet, seed: &WallSeed) -> Self {
        let variant = tile_set.variant();
        let mut tiles = tile_set.into_tiles();
        let mut random = ChaCha20Rng::from_seed(seed.0);
        fisher_yates_shuffle(&mut tiles, &mut random);

        let live_end = tiles
            .len()
            .checked_sub(DEAD_WALL_SIZE)
            .expect("a valid riichi tile set always contains a dead wall");
        Self {
            variant,
            tiles: tiles.into_boxed_slice(),
            live_draw_index: 0,
            live_end,
            rinshan_draws: 0,
            revealed_dora_count: 1,
        }
    }

    #[must_use]
    pub const fn variant(&self) -> RiichiVariant {
        self.variant
    }

    #[must_use]
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    #[must_use]
    pub fn remaining_live_draws(&self) -> usize {
        self.live_end
            .saturating_sub(self.live_draw_index)
            .saturating_sub(usize::from(self.rinshan_draws))
    }

    pub fn draw_live(&mut self) -> Option<Tile> {
        if self.remaining_live_draws() == 0 {
            return None;
        }

        let tile = self.tiles[self.live_draw_index];
        self.live_draw_index += 1;
        Some(tile)
    }

    pub fn draw_rinshan(&mut self) -> Result<Tile, WallError> {
        if self.rinshan_draws >= MAX_RINSHAN_DRAWS {
            return Err(WallError::RinshanExhausted);
        }
        if self.remaining_live_draws() == 0 {
            return Err(WallError::LiveWallExhausted);
        }

        let offset = RINSHAN_OFFSETS[usize::from(self.rinshan_draws)];
        let tile = self.dead_wall_tile(offset);
        self.rinshan_draws += 1;
        Ok(tile)
    }

    #[must_use]
    pub const fn rinshan_draw_count(&self) -> u8 {
        self.rinshan_draws
    }

    #[must_use]
    pub const fn revealed_dora_count(&self) -> u8 {
        self.revealed_dora_count
    }

    #[must_use]
    pub(crate) const fn live_draw_count(&self) -> usize {
        self.live_draw_index
    }

    #[must_use]
    pub(crate) fn tile_by_id(&self, tile_id: crate::TileId) -> Option<Tile> {
        self.tiles.iter().copied().find(|tile| tile.id() == tile_id)
    }

    #[must_use]
    pub fn current_dora_indicators(&self) -> impl ExactSizeIterator<Item = Tile> + '_ {
        DORA_OFFSETS[..usize::from(self.revealed_dora_count)]
            .iter()
            .map(|offset| self.dead_wall_tile(*offset))
    }

    pub fn reveal_next_dora(&mut self) -> Result<Tile, WallError> {
        if self.revealed_dora_count >= MAX_DORA_INDICATORS {
            return Err(WallError::DoraIndicatorsExhausted);
        }

        let offset = DORA_OFFSETS[usize::from(self.revealed_dora_count)];
        self.revealed_dora_count += 1;
        Ok(self.dead_wall_tile(offset))
    }

    #[must_use]
    pub fn matching_ura_dora_indicators(&self) -> impl ExactSizeIterator<Item = Tile> + '_ {
        URA_DORA_OFFSETS[..usize::from(self.revealed_dora_count)]
            .iter()
            .map(|offset| self.dead_wall_tile(*offset))
    }

    fn dead_wall_tile(&self, offset: usize) -> Tile {
        self.tiles[self.live_end + offset]
    }

    #[cfg(test)]
    fn secret_order(&self) -> &[Tile] {
        &self.tiles
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WallError {
    RinshanExhausted,
    DoraIndicatorsExhausted,
    LiveWallExhausted,
}

impl Display for WallError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::RinshanExhausted => "all four rinshan tiles have been drawn",
            Self::DoraIndicatorsExhausted => "all five dora indicators have been revealed",
            Self::LiveWallExhausted => "the live wall has no drawable tiles",
        };
        formatter.write_str(message)
    }
}

impl Error for WallError {}

fn fisher_yates_shuffle<T>(values: &mut [T], random: &mut impl RngCore) {
    for upper_index in (1..values.len()).rev() {
        let bound = u64::try_from(upper_index + 1).expect("slice length fits into u64");
        let selected = usize::try_from(uniform_below(random, bound))
            .expect("sampled index is bounded by the slice length");
        values.swap(upper_index, selected);
    }
}

fn uniform_below(random: &mut impl RngCore, bound: u64) -> u64 {
    debug_assert!(bound > 0);
    let rejection_threshold = bound.wrapping_neg() % bound;
    loop {
        let value = random.next_u64();
        if value >= rejection_threshold {
            return value % bound;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{RiichiVariant, TileId, TileSet};

    use super::{Wall, WallError, WallSeed};

    fn fixed_seed(value: u8) -> WallSeed {
        WallSeed::from_bytes([value; 32])
    }

    #[test]
    fn seed_debug_output_never_reveals_secret_bytes() {
        let seed = fixed_seed(0xab);
        let output = format!("{seed:?}");

        assert_eq!(output, "WallSeed([REDACTED])");
        assert!(!output.contains("ab"));
    }

    #[test]
    fn production_seed_comes_from_operating_system() {
        let seed = WallSeed::generate().expect("operating-system random source is available");

        assert_eq!(seed.expose_bytes().len(), 32);
    }

    #[test]
    fn same_seed_reproduces_identical_wall() {
        let seed = fixed_seed(7);
        let first = Wall::new(TileSet::standard(RiichiVariant::Yonma), &seed);
        let second = Wall::new(TileSet::standard(RiichiVariant::Yonma), &seed);

        assert_eq!(first.secret_order(), second.secret_order());
    }

    #[test]
    fn different_seed_changes_wall_order() {
        let first = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(1));
        let second = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(2));

        assert_ne!(first.secret_order(), second.secret_order());
    }

    #[test]
    fn fixed_seed_has_stable_shuffle_fixture() {
        let wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(0));
        let first_ids: Vec<_> = wall
            .secret_order()
            .iter()
            .take(12)
            .map(|tile| tile.id().value())
            .collect();

        assert_eq!(first_ids, [84, 8, 6, 17, 15, 33, 50, 80, 135, 76, 52, 108]);
    }

    #[test]
    fn variants_reserve_fourteen_tile_dead_wall() {
        for variant in [RiichiVariant::Yonma, RiichiVariant::Sanma] {
            let wall = Wall::new(TileSet::standard(variant), &fixed_seed(3));

            assert_eq!(wall.tile_count(), variant.tile_count());
            assert_eq!(wall.remaining_live_draws(), variant.tile_count() - 14);
            assert_eq!(wall.current_dora_indicators().len(), 1);
            assert_eq!(wall.matching_ura_dora_indicators().len(), 1);
        }
    }

    #[test]
    fn live_wall_draws_each_tile_at_most_once() {
        let mut wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(4));
        let mut ids = HashSet::new();

        while let Some(tile) = wall.draw_live() {
            assert!(ids.insert(tile.id()));
        }

        assert_eq!(ids.len(), 122);
        assert_eq!(wall.remaining_live_draws(), 0);
        assert_eq!(wall.draw_live(), None);
    }

    #[test]
    fn each_rinshan_draw_reduces_live_draw_capacity() {
        let mut wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(5));
        let initial = wall.remaining_live_draws();
        let mut rinshan_ids = HashSet::new();

        for expected_count in 1..=4 {
            let tile = wall.draw_rinshan().expect("rinshan tile available");
            assert!(rinshan_ids.insert(tile.id()));
            assert_eq!(wall.rinshan_draw_count(), expected_count);
            assert_eq!(
                wall.remaining_live_draws(),
                initial - usize::from(expected_count)
            );
        }

        assert_eq!(wall.draw_rinshan(), Err(WallError::RinshanExhausted));
    }

    #[test]
    fn dora_and_ura_indicators_are_paired_and_bounded() {
        let mut wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &fixed_seed(6));
        let mut indicator_ids: HashSet<TileId> = wall
            .current_dora_indicators()
            .map(|tile| tile.id())
            .collect();

        for expected_count in 2..=5 {
            let tile = wall.reveal_next_dora().expect("indicator available");
            assert!(indicator_ids.insert(tile.id()));
            assert_eq!(wall.revealed_dora_count(), expected_count);
            assert_eq!(
                wall.matching_ura_dora_indicators().len(),
                usize::from(expected_count)
            );
        }

        assert_eq!(
            wall.reveal_next_dora(),
            Err(WallError::DoraIndicatorsExhausted)
        );
        let ura_ids: HashSet<_> = wall
            .matching_ura_dora_indicators()
            .map(|tile| tile.id())
            .collect();
        assert!(indicator_ids.is_disjoint(&ura_ids));
    }

    #[test]
    fn dead_wall_partitions_do_not_overlap() {
        let seed = fixed_seed(9);
        let mut wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &seed);
        for _ in 1..5 {
            wall.reveal_next_dora().expect("indicator available");
        }
        let indicators: HashSet<_> = wall
            .current_dora_indicators()
            .chain(wall.matching_ura_dora_indicators())
            .map(|tile| tile.id())
            .collect();
        let rinshan: HashSet<_> = (0..4)
            .map(|_| wall.draw_rinshan().expect("rinshan available").id())
            .collect();

        assert_eq!(indicators.len(), 10);
        assert_eq!(rinshan.len(), 4);
        assert!(indicators.is_disjoint(&rinshan));

        let mut live_wall = Wall::new(TileSet::standard(RiichiVariant::Yonma), &seed);
        let live_ids: HashSet<_> = std::iter::from_fn(|| live_wall.draw_live())
            .map(|tile| tile.id())
            .collect();
        assert!(indicators.is_disjoint(&live_ids));
        assert!(rinshan.is_disjoint(&live_ids));
    }
}
