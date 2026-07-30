use crate::{EndReason, Seat, TableProgress, Tile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawSource {
    LiveWall,
    Rinshan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionKind {
    Pass,
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
    ExhaustiveDrawDeclared {
        reason: EndReason,
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
