use crate::{Seat, Tile, TileId, TileKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MeldId(u8);

impl MeldId {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MeldKind {
    Chi,
    Pon,
    OpenKan,
    ConcealedKan,
    AddedKan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Meld {
    pub(super) id: MeldId,
    pub(super) kind: MeldKind,
    pub(super) tiles: Box<[Tile]>,
    pub(super) called_from: Option<Seat>,
    pub(super) called_tile: Option<TileId>,
}

impl Meld {
    pub(super) fn new(
        id: MeldId,
        kind: MeldKind,
        tiles: impl Into<Box<[Tile]>>,
        called_from: Option<Seat>,
        called_tile: Option<TileId>,
    ) -> Self {
        Self {
            id,
            kind,
            tiles: tiles.into(),
            called_from,
            called_tile,
        }
    }

    #[must_use]
    pub const fn id(&self) -> MeldId {
        self.id
    }

    #[must_use]
    pub const fn kind(&self) -> MeldKind {
        self.kind
    }

    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    #[must_use]
    pub const fn called_from(&self) -> Option<Seat> {
        self.called_from
    }

    #[must_use]
    pub const fn called_tile(&self) -> Option<TileId> {
        self.called_tile
    }

    #[must_use]
    pub fn tile_kind(&self) -> TileKind {
        self.tiles[0].kind()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discard {
    tile: Tile,
    tsumogiri: bool,
    riichi_declared: bool,
    claimed_by: Option<Seat>,
}

impl Discard {
    pub(super) const fn new(tile: Tile, tsumogiri: bool) -> Self {
        Self {
            tile,
            tsumogiri,
            riichi_declared: false,
            claimed_by: None,
        }
    }

    #[must_use]
    pub const fn tile(&self) -> Tile {
        self.tile
    }

    #[must_use]
    pub const fn is_tsumogiri(&self) -> bool {
        self.tsumogiri
    }

    #[must_use]
    pub const fn is_riichi_declaration(&self) -> bool {
        self.riichi_declared
    }

    #[must_use]
    pub const fn claimed_by(&self) -> Option<Seat> {
        self.claimed_by
    }

    pub(super) fn mark_claimed(&mut self, seat: Seat) {
        debug_assert!(self.claimed_by.is_none());
        self.claimed_by = Some(seat);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RiichiStatus {
    #[default]
    None,
    Pending,
    Established,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reaction {
    Pass,
    Chi { hand_tiles: [TileId; 2] },
    Pon { hand_tiles: [TileId; 2] },
    OpenKan { hand_tiles: [TileId; 3] },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerHand {
    pub(super) points: i32,
    pub(super) concealed: Vec<Tile>,
    pub(super) drawn_tile: Option<TileId>,
    pub(super) melds: Vec<Meld>,
    pub(super) discards: Vec<Discard>,
    pub(super) riichi: RiichiStatus,
    pub(super) temporary_furiten: bool,
    pub(super) riichi_furiten: bool,
    pub(super) ippatsu_eligible: bool,
}

impl PlayerHand {
    pub(super) fn new(points: i32) -> Self {
        Self {
            points,
            concealed: Vec::with_capacity(14),
            drawn_tile: None,
            melds: Vec::with_capacity(4),
            discards: Vec::with_capacity(24),
            riichi: RiichiStatus::None,
            temporary_furiten: false,
            riichi_furiten: false,
            ippatsu_eligible: false,
        }
    }

    #[must_use]
    pub const fn points(&self) -> i32 {
        self.points
    }

    #[must_use]
    pub fn concealed(&self) -> &[Tile] {
        &self.concealed
    }

    #[must_use]
    pub const fn drawn_tile_id(&self) -> Option<TileId> {
        self.drawn_tile
    }

    #[must_use]
    pub fn melds(&self) -> &[Meld] {
        &self.melds
    }

    #[must_use]
    pub fn discards(&self) -> &[Discard] {
        &self.discards
    }

    #[must_use]
    pub const fn riichi_status(&self) -> RiichiStatus {
        self.riichi
    }

    #[must_use]
    pub const fn is_temporary_furiten(&self) -> bool {
        self.temporary_furiten
    }

    #[must_use]
    pub const fn is_riichi_furiten(&self) -> bool {
        self.riichi_furiten
    }

    #[must_use]
    pub const fn is_ippatsu_eligible(&self) -> bool {
        self.ippatsu_eligible
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndReason {
    ExhaustiveDraw,
    NineTerminals,
    FourWinds,
    FourKans,
    FourRiichi,
    Tsumo,
    Ron,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandPhase {
    AwaitingTurnAction { seat: Seat },
    AwaitingDiscard { seat: Seat },
    AwaitingResponses { trigger_seat: Seat },
    Ended { reason: EndReason },
}
