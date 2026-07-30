use crate::{DrawSource, MeldId, PlayerHand, RiichiRules, Seat, TableProgress, Tile, TileId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinSource {
    Tsumo(DrawSource),
    Discard { from: Seat },
    AddedKan { from: Seat, meld_id: MeldId },
    ConcealedKan { from: Seat },
}

#[derive(Clone, Copy, Debug)]
pub struct WinQuery<'a> {
    rules: &'a RiichiRules,
    progress: TableProgress,
    seat: Seat,
    player: &'a PlayerHand,
    winning_tile: Tile,
    source: WinSource,
}

impl<'a> WinQuery<'a> {
    pub(super) const fn new(
        rules: &'a RiichiRules,
        progress: TableProgress,
        seat: Seat,
        player: &'a PlayerHand,
        winning_tile: Tile,
        source: WinSource,
    ) -> Self {
        Self {
            rules,
            progress,
            seat,
            player,
            winning_tile,
            source,
        }
    }

    #[must_use]
    pub const fn rules(self) -> &'a RiichiRules {
        self.rules
    }

    #[must_use]
    pub const fn progress(self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn seat(self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn player(self) -> &'a PlayerHand {
        self.player
    }

    #[must_use]
    pub const fn winning_tile(self) -> Tile {
        self.winning_tile
    }

    #[must_use]
    pub const fn source(self) -> WinSource {
        self.source
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RiichiQuery<'a> {
    rules: &'a RiichiRules,
    progress: TableProgress,
    seat: Seat,
    player: &'a PlayerHand,
    discard_tile: Tile,
}

impl<'a> RiichiQuery<'a> {
    pub(super) const fn new(
        rules: &'a RiichiRules,
        progress: TableProgress,
        seat: Seat,
        player: &'a PlayerHand,
        discard_tile: Tile,
    ) -> Self {
        Self {
            rules,
            progress,
            seat,
            player,
            discard_tile,
        }
    }

    #[must_use]
    pub const fn rules(self) -> &'a RiichiRules {
        self.rules
    }

    #[must_use]
    pub const fn progress(self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn seat(self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn player(self) -> &'a PlayerHand {
        self.player
    }

    #[must_use]
    pub const fn discard_tile(self) -> Tile {
        self.discard_tile
    }
}

#[derive(Clone, Copy, Debug)]
pub struct KanQuery<'a> {
    rules: &'a RiichiRules,
    progress: TableProgress,
    seat: Seat,
    player: &'a PlayerHand,
    tile_ids: [TileId; 4],
}

impl<'a> KanQuery<'a> {
    pub(super) const fn new(
        rules: &'a RiichiRules,
        progress: TableProgress,
        seat: Seat,
        player: &'a PlayerHand,
        tile_ids: [TileId; 4],
    ) -> Self {
        Self {
            rules,
            progress,
            seat,
            player,
            tile_ids,
        }
    }

    #[must_use]
    pub const fn rules(self) -> &'a RiichiRules {
        self.rules
    }

    #[must_use]
    pub const fn progress(self) -> TableProgress {
        self.progress
    }

    #[must_use]
    pub const fn seat(self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn player(self) -> &'a PlayerHand {
        self.player
    }

    #[must_use]
    pub const fn tile_ids(self) -> [TileId; 4] {
        self.tile_ids
    }
}

pub trait HandJudge: Send + Sync {
    fn can_win(&self, _query: WinQuery<'_>) -> bool {
        false
    }

    fn can_riichi(&self, _query: RiichiQuery<'_>) -> bool {
        false
    }

    fn can_concealed_kan_after_riichi(&self, _query: KanQuery<'_>) -> bool {
        false
    }

    fn is_tenpai(&self, _rules: &RiichiRules, _player: &PlayerHand, _seat: Seat) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RejectAllHandJudge;

impl HandJudge for RejectAllHandJudge {}
