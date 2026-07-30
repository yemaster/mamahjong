use crate::{EndReason, Meld, MeldKind, Seat, TableProgress, Tile, TileKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawSource {
    LiveWall,
    Rinshan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionKind {
    Pass,
    Ron,
    Chi,
    Pon,
    OpenKan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandEvent {
    HandStarted {
        progress: TableProgress,
        dora_indicator: Tile,
        remaining_live_draws: usize,
    },
    InitialHandDealt {
        seat: Seat,
        tiles: Box<[Tile]>,
    },
    TileDrawn {
        seat: Seat,
        tile: Tile,
        source: DrawSource,
        remaining_live_draws: usize,
    },
    TileDiscarded {
        seat: Seat,
        tile: Tile,
        tsumogiri: bool,
        riichi_declared: bool,
    },
    ReactionSubmitted {
        seat: Seat,
        reaction: ReactionKind,
    },
    MeldDeclared {
        seat: Seat,
        meld: Meld,
    },
    KanProposed {
        seat: Seat,
        kind: MeldKind,
        tile_kind: TileKind,
    },
    KanCompleted {
        seat: Seat,
        meld: Meld,
    },
    DoraIndicatorRevealed {
        tile: Tile,
        revealed_count: u8,
    },
    RiichiEstablished {
        seat: Seat,
        points_after: i32,
        riichi_sticks: u32,
    },
    IppatsuExpired {
        seat: Seat,
    },
    IppatsuCancelled {
        seats: Box<[Seat]>,
    },
    FuritenChanged {
        seat: Seat,
        temporary: bool,
        riichi: bool,
    },
    TsumoDeclared {
        winner: Seat,
        tile: Tile,
        source: DrawSource,
    },
    RonDeclared {
        winners: Box<[Seat]>,
        from: Seat,
        tile: Tile,
        source: crate::WinSource,
    },
    AbortiveDrawDeclared {
        reason: EndReason,
        declarer: Option<Seat>,
    },
    ExhaustiveDrawDeclared {
        reason: EndReason,
        tenpai: Box<[Seat]>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HandTransition {
    events: Box<[HandEvent]>,
}

impl HandTransition {
    #[must_use]
    pub(crate) fn new(events: impl Into<Box<[HandEvent]>>) -> Self {
        Self {
            events: events.into(),
        }
    }

    pub(crate) fn append(&mut self, other: Self) {
        let mut combined = std::mem::take(&mut self.events).into_vec();
        combined.reserve(other.events.len());
        combined.extend(other.events);
        self.events = combined.into_boxed_slice();
    }

    #[must_use]
    pub fn events(&self) -> &[HandEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Box<[HandEvent]> {
        self.events
    }
}
