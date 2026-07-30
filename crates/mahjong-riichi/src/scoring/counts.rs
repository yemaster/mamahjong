use crate::{Tile, TileKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TileCounts {
    values: [u8; TileKind::COUNT],
    total: u8,
}

impl TileCounts {
    pub(super) const fn empty() -> Self {
        Self {
            values: [0; TileKind::COUNT],
            total: 0,
        }
    }

    pub(super) fn from_tiles<'a>(
        tiles: impl IntoIterator<Item = &'a Tile>,
    ) -> Result<Self, CountError> {
        let mut counts = Self::empty();
        for tile in tiles {
            counts.add(tile.kind())?;
        }
        Ok(counts)
    }

    pub(super) fn add(&mut self, kind: TileKind) -> Result<(), CountError> {
        let value = &mut self.values[kind.index()];
        if *value >= 4 {
            return Err(CountError::TooManyCopies { kind });
        }
        *value += 1;
        self.total = self
            .total
            .checked_add(1)
            .expect("a mahjong hand contains fewer than 256 tiles");
        Ok(())
    }

    pub(super) fn remove(&mut self, kind: TileKind) -> bool {
        let value = &mut self.values[kind.index()];
        if *value == 0 {
            return false;
        }
        *value -= 1;
        self.total -= 1;
        true
    }

    pub(super) const fn get(&self, kind: TileKind) -> u8 {
        self.values[kind.index()]
    }

    pub(super) const fn total(&self) -> u8 {
        self.total
    }

    pub(super) fn first_present(&self) -> Option<TileKind> {
        self.values
            .iter()
            .position(|count| *count > 0)
            .map(|index| {
                TileKind::from_index(u8::try_from(index).expect("tile kind index fits u8"))
                    .expect("count array only contains tile kinds")
            })
    }

    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = (TileKind, u8)> + '_ {
        self.values.iter().enumerate().map(|(index, count)| {
            (
                TileKind::from_index(u8::try_from(index).expect("tile kind index fits u8"))
                    .expect("count array only contains tile kinds"),
                *count,
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CountError {
    TooManyCopies { kind: TileKind },
}

#[cfg(test)]
mod tests {
    use crate::{Tile, TileId, TileKind};

    use super::TileCounts;

    #[test]
    fn rejects_a_fifth_copy_without_mutating_count() {
        let kind: TileKind = "1m".parse().expect("kind");
        let tiles: Vec<_> = (0..4)
            .map(|id| Tile::new(TileId::new(id), kind, false).expect("tile"))
            .collect();
        let mut counts = TileCounts::from_tiles(&tiles).expect("four copies");

        assert!(counts.add(kind).is_err());
        assert_eq!(counts.get(kind), 4);
        assert_eq!(counts.total(), 4);
    }
}
