mod counts;
mod shape;

pub use shape::WaitingTiles;

use crate::{HandJudge, KanQuery, PlayerHand, RiichiQuery};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiichiScorer;

impl RiichiScorer {
    #[must_use]
    pub fn waiting_tiles(self, player: &PlayerHand) -> WaitingTiles {
        shape::waiting_tiles(player.concealed(), player.melds().len())
    }
}

impl HandJudge for RiichiScorer {
    fn can_riichi(&self, query: RiichiQuery<'_>) -> bool {
        let concealed: Vec<_> = query
            .player()
            .concealed()
            .iter()
            .copied()
            .filter(|tile| tile.id() != query.discard_tile().id())
            .collect();
        if concealed.len() + 1 != query.player().concealed().len() {
            return false;
        }
        let waits = shape::waiting_tiles(&concealed, query.player().melds().len());
        waits
            .kinds()
            .iter()
            .any(|kind| known_kind_count(query.player(), *kind) < 4)
    }

    fn can_concealed_kan_after_riichi(&self, query: KanQuery<'_>) -> bool {
        let Some(drawn_tile_id) = query.player().drawn_tile_id() else {
            return false;
        };
        let before: Vec<_> = query
            .player()
            .concealed()
            .iter()
            .copied()
            .filter(|tile| tile.id() != drawn_tile_id)
            .collect();
        let after: Vec<_> = query
            .player()
            .concealed()
            .iter()
            .copied()
            .filter(|tile| !query.tile_ids().contains(&tile.id()))
            .collect();
        if before.len() + 1 != query.player().concealed().len()
            || after.len() + 4 != query.player().concealed().len()
        {
            return false;
        }
        shape::waiting_tiles(&before, query.player().melds().len())
            == shape::waiting_tiles(&after, query.player().melds().len() + 1)
    }

    fn is_tenpai(
        &self,
        _rules: &crate::RiichiRules,
        player: &PlayerHand,
        _seat: crate::Seat,
    ) -> bool {
        !self.waiting_tiles(player).is_empty()
    }
}

fn known_kind_count(player: &PlayerHand, kind: crate::TileKind) -> usize {
    let concealed = player
        .concealed()
        .iter()
        .filter(|tile| tile.kind() == kind)
        .count();
    let melds = player
        .melds()
        .iter()
        .flat_map(|meld| meld.tiles())
        .filter(|tile| tile.kind() == kind)
        .count();
    let discards = player
        .discards()
        .iter()
        .filter(|discard| discard.tile().kind() == kind)
        .count();
    concealed + melds + discards
}
