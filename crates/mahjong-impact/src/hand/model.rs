//! 单局里的值对象：副露、牌河、和牌来源、阶段。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::progress::Seat;
use crate::tile::{Tile, TileId, TileKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MeldId(u16);

impl MeldId {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// 副露种类。吃只在亮子麻将开放。
///
/// `IndicatorPon` / `IndicatorConcealed` 是「指示牌碰牌算杠」开启后的两种特例：
/// **杠点按明杠 / 暗杠结算，但牌型仍然是刻子**——不摸岭上牌、不算杠上开花、
/// 也不计入三杠。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MeldKind {
    Chi,
    Pon,
    OpenKan,
    ConcealedKan,
    AddedKan,
    IndicatorPon,
    IndicatorConcealed,
}

impl MeldKind {
    /// 是不是真正的杠：决定摸岭上牌、杠上开花与三杠计数。
    #[must_use]
    pub const fn is_kan(self) -> bool {
        matches!(self, Self::OpenKan | Self::ConcealedKan | Self::AddedKan)
    }

    /// 这组副露实际占几张牌。
    #[must_use]
    pub const fn tile_count(self) -> u8 {
        if self.is_kan() { 4 } else { 3 }
    }

    /// 杠完之后要不要从牌山末尾补一张。
    #[must_use]
    pub const fn draws_replacement(self) -> bool {
        self.is_kan()
    }

    /// 是不是暗的（结算杠点时按暗杠走）。
    #[must_use]
    pub const fn is_concealed(self) -> bool {
        matches!(self, Self::ConcealedKan | Self::IndicatorConcealed)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chi => "chi",
            Self::Pon => "pon",
            Self::OpenKan => "open_kan",
            Self::ConcealedKan => "concealed_kan",
            Self::AddedKan => "added_kan",
            Self::IndicatorPon => "indicator_pon",
            Self::IndicatorConcealed => "indicator_concealed",
        }
    }
}

impl Display for MeldKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Meld {
    id: MeldId,
    kind: MeldKind,
    tile: TileKind,
    tiles: Vec<Tile>,
    /// 鸣牌来源。暗杠 / 指示牌暗杠没有来源。
    called_from: Option<Seat>,
    /// 被鸣的那一张。
    called_tile: Option<TileId>,
}

impl Meld {
    #[must_use]
    pub fn new(
        id: MeldId,
        kind: MeldKind,
        tile: TileKind,
        tiles: Vec<Tile>,
        called_from: Option<Seat>,
        called_tile: Option<TileId>,
    ) -> Self {
        debug_assert_eq!(tiles.len(), usize::from(kind.tile_count()));
        debug_assert!(kind == MeldKind::Chi || tiles.iter().all(|held| held.kind() == tile));
        Self {
            id,
            kind,
            tile,
            tiles,
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
    pub const fn tile(&self) -> TileKind {
        self.tile
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

    /// 碰 → 加杠：补上第四张，种类换成 `AddedKan`。
    pub(crate) fn upgrade_to_added_kan(&mut self, tile: Tile) {
        debug_assert_eq!(self.kind, MeldKind::Pon);
        debug_assert_eq!(tile.kind(), self.tile);
        self.kind = MeldKind::AddedKan;
        self.tiles.push(tile);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Discard {
    tile: Tile,
    /// 被别家鸣走了。
    called: bool,
}

impl Discard {
    #[must_use]
    pub const fn new(tile: Tile) -> Self {
        Self {
            tile,
            called: false,
        }
    }

    #[must_use]
    pub const fn tile(self) -> Tile {
        self.tile
    }

    #[must_use]
    pub const fn called(self) -> bool {
        self.called
    }

    pub(crate) const fn mark_called(&mut self) {
        self.called = true;
    }
}

/// 这张牌从哪里摸来的。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawSource {
    /// 牌山正常摸牌。
    Wall,
    /// 杠之后从牌山末尾补的岭上牌。
    Replacement,
}

/// 单局结束的原因。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndReason {
    /// 自摸和牌。
    Tsumo,
    /// 荣和（含抢杠）。
    Ron,
    /// 牌山摸完无人和牌：本局不算，同一庄重开。
    ExhaustiveDraw,
}

impl EndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tsumo => "tsumo",
            Self::Ron => "ron",
            Self::ExhaustiveDraw => "exhaustive_draw",
        }
    }
}

/// 单局阶段。四个取值与前端既有的 `MatchPhase` 联合一一对应。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandPhase {
    /// 轮到 `seat` 行动：可以自摸、暗杠、加杠、指示牌暗杠，或打牌。
    AwaitingTurnAction {
        seat: Seat,
    },
    /// 轮到 `seat` 打牌（鸣牌之后只能打）。
    AwaitingDiscard {
        seat: Seat,
    },
    /// 等其余三家对 `discarder` 打出的牌做出反应。
    AwaitingResponses {
        discarder: Seat,
    },
    /// 杠完之后等四家都播完杠点动画，播完再由 `seat` 摸岭上牌。
    AwaitingKanAnimation {
        seat: Seat,
    },
    Ended {
        reason: EndReason,
    },
}

impl HandPhase {
    /// 现在轮到谁动（等待响应时没有唯一的行动者）。
    #[must_use]
    pub const fn actor(self) -> Option<Seat> {
        match self {
            Self::AwaitingTurnAction { seat } | Self::AwaitingDiscard { seat } => Some(seat),
            Self::AwaitingResponses { .. }
            | Self::AwaitingKanAnimation { .. }
            | Self::Ended { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_ended(self) -> bool {
        matches!(self, Self::Ended { .. })
    }
}

/// 对别家打出的牌（或加杠牌）可以做的反应。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionKind {
    Ron,
    Chi { hand_tiles: [TileId; 2] },
    Pon,
    OpenKan,
    Pass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reaction {
    seat: Seat,
    kind: ReactionKind,
}

impl Reaction {
    #[must_use]
    pub const fn new(seat: Seat, kind: ReactionKind) -> Self {
        Self { seat, kind }
    }

    #[must_use]
    pub const fn seat(self) -> Seat {
        self.seat
    }

    #[must_use]
    pub const fn kind(self) -> ReactionKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandError {
    /// 现在不该这个座位动。
    OutOfTurn { seat: Seat },
    /// 当前阶段不接受这个动作。
    UnexpectedAction,
    /// 手里没有这张牌。
    TileNotHeld { tile: TileId },
    /// 凑不齐这组副露。
    MeldNotAvailable,
    /// 找不到这组副露。
    MeldNotFound { meld: MeldId },
    /// 手牌没和。
    NotAWinningHand,
    /// 牌山空了。
    WallExhausted,
    /// 本局已经结束。
    HandAlreadyEnded,
}

impl Display for HandError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfTurn { seat } => {
                write!(formatter, "seat {} cannot act right now", seat.index())
            }
            Self::UnexpectedAction => formatter.write_str("action is not legal in this phase"),
            Self::TileNotHeld { tile } => {
                write!(formatter, "tile {} is not in hand", tile.value())
            }
            Self::MeldNotAvailable => formatter.write_str("the hand cannot form that meld"),
            Self::MeldNotFound { meld } => {
                write!(formatter, "meld {} does not exist", meld.value())
            }
            Self::NotAWinningHand => formatter.write_str("the hand is not complete"),
            Self::WallExhausted => formatter.write_str("the wall has no drawable tiles left"),
            Self::HandAlreadyEnded => formatter.write_str("the hand has already ended"),
        }
    }
}

impl Error for HandError {}

#[cfg(test)]
mod tests {
    use super::{Discard, MeldKind};
    use crate::tile::{Tile, TileId, TileKind};

    #[test]
    fn only_real_kans_draw_a_replacement_tile() {
        assert!(MeldKind::OpenKan.draws_replacement());
        assert!(MeldKind::ConcealedKan.draws_replacement());
        assert!(MeldKind::AddedKan.draws_replacement());
        assert!(!MeldKind::Pon.draws_replacement());
        assert!(!MeldKind::IndicatorPon.draws_replacement());
        assert!(!MeldKind::IndicatorConcealed.draws_replacement());
    }

    #[test]
    fn indicator_melds_stay_three_tile_triplets() {
        assert_eq!(MeldKind::IndicatorPon.tile_count(), 3);
        assert_eq!(MeldKind::IndicatorConcealed.tile_count(), 3);
        assert!(!MeldKind::IndicatorPon.is_kan());
        assert!(!MeldKind::IndicatorConcealed.is_kan());
        assert!(MeldKind::IndicatorConcealed.is_concealed());
        assert!(!MeldKind::IndicatorPon.is_concealed());
    }

    #[test]
    fn discards_remember_whether_they_were_called() {
        let kind: TileKind = "1z".parse().expect("valid tile code");
        let mut discard = Discard::new(Tile::new(TileId::new(0), kind));

        assert!(!discard.called());
        discard.mark_called();
        assert!(discard.called());
    }
}
