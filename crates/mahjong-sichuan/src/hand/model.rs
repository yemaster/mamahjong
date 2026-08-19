//! 单局里的值对象：副露、牌河、阶段、反应。

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

/// 副露种类。四川麻将没有吃，只有碰与三种杠。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MeldKind {
    Pon,
    OpenKan,
    ConcealedKan,
    AddedKan,
}

impl MeldKind {
    /// 是不是杠：杠算「根」，也决定摸岭上牌。
    #[must_use]
    pub const fn is_kan(self) -> bool {
        matches!(self, Self::OpenKan | Self::ConcealedKan | Self::AddedKan)
    }

    /// 这组副露实际占几张牌。
    #[must_use]
    pub const fn tile_count(self) -> u8 {
        if self.is_kan() { 4 } else { 3 }
    }

    /// 是不是暗的（结算杠点按暗杠走）。
    #[must_use]
    pub const fn is_concealed(self) -> bool {
        matches!(self, Self::ConcealedKan)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pon => "pon",
            Self::OpenKan => "open_kan",
            Self::ConcealedKan => "concealed_kan",
            Self::AddedKan => "added_kan",
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
    /// 鸣牌来源。暗杠没有来源。
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
        debug_assert!(tiles.iter().all(|held| held.kind() == tile));
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
    /// 三家胡，本局结束。
    ThreeWinners,
    /// 牌山摸完未满三家胡：流局（查花猪 + 查大叫）。
    ExhaustiveDraw,
}

impl EndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThreeWinners => "three_winners",
            Self::ExhaustiveDraw => "exhaustive_draw",
        }
    }
}

/// 单局阶段。换三张 / 定缺两阶段是「全体各自表态」，没有单一行动者。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandPhase {
    /// 每家选 3 张同花色牌，沿骰子决定的方向传给接收家。
    AwaitingExchange,
    /// 四家换牌动画完成前的同步门；完成后才进入定缺选择。
    AwaitingExchangeAnimation,
    /// 每家选一门定缺。
    AwaitingDingQue,
    /// 轮到 `seat` 行动：可以自摸、暗杠、加杠，或打牌。
    AwaitingTurnAction {
        seat: Seat,
    },
    /// 轮到 `seat` 打牌（鸣牌之后只能打）。
    AwaitingDiscard {
        seat: Seat,
    },
    /// 等其余家对 `discarder` 打出的牌做出反应。
    AwaitingResponses {
        discarder: Seat,
    },
    /// 杠完之后等四家都播完杠点动画，播完再由 `seat` 摸岭上牌。
    AwaitingKanAnimation {
        seat: Seat,
    },
    /// 胡牌后等待所有客户端播放点数/胡牌动画；动画结束后才继续血战或结算。
    AwaitingWinAnimation {
        seat: Seat,
    },
    Ended {
        reason: EndReason,
    },
}

impl HandPhase {
    /// 现在轮到谁动（等待响应 / 换三张 / 定缺 / 杠动画时没有唯一行动者）。
    #[must_use]
    pub const fn actor(self) -> Option<Seat> {
        match self {
            Self::AwaitingTurnAction { seat } | Self::AwaitingDiscard { seat } => Some(seat),
            Self::AwaitingExchange
            | Self::AwaitingExchangeAnimation
            | Self::AwaitingDingQue
            | Self::AwaitingResponses { .. }
            | Self::AwaitingKanAnimation { .. }
            | Self::AwaitingWinAnimation { .. }
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// 换三张必须选 3 张。
    ExchangeWrongCount { actual: usize },
    /// 换三张的三张必须同花色。
    ExchangeTilesNotSameSuit,
    /// 换三张里有一张不在手上。
    ExchangeTileNotHeld { tile: TileId },
    /// 换三张选了三张里有重复的。
    ExchangeDuplicateTile,
    /// 手上还有定缺门，必须先打定缺门。
    QueTilesRemaining,
    /// 开发模式给了一个认不出的牌码。
    InvalidTileCode(String),
    /// 开发模式给的牌码张数和暗手对不上。
    WrongConcealedTileCount { expected: usize, actual: usize },
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
            Self::ExchangeWrongCount { actual } => {
                write!(formatter, "exchange needs 3 tiles, got {actual}")
            }
            Self::ExchangeTilesNotSameSuit => {
                formatter.write_str("the three exchange tiles must share a suit")
            }
            Self::ExchangeTileNotHeld { tile } => {
                write!(formatter, "exchange tile {} is not in hand", tile.value())
            }
            Self::ExchangeDuplicateTile => {
                formatter.write_str("the three exchange tiles must be distinct")
            }
            Self::QueTilesRemaining => formatter.write_str("discard the que suit first"),
            Self::InvalidTileCode(code) => write!(formatter, "invalid tile code {code}"),
            Self::WrongConcealedTileCount { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} concealed tiles, got {actual}"
                )
            }
        }
    }
}

impl Error for HandError {}

#[cfg(test)]
mod tests {
    use super::{Discard, MeldKind};
    use crate::tile::{Tile, TileId, TileKind};

    #[test]
    fn only_kans_draw_a_replacement_tile() {
        assert!(MeldKind::OpenKan.is_kan());
        assert!(MeldKind::ConcealedKan.is_kan());
        assert!(MeldKind::AddedKan.is_kan());
        assert!(!MeldKind::Pon.is_kan());
        assert_eq!(MeldKind::Pon.tile_count(), 3);
        assert_eq!(MeldKind::ConcealedKan.tile_count(), 4);
    }

    #[test]
    fn discards_remember_whether_they_were_called() {
        let kind: TileKind = "1m".parse().expect("valid tile code");
        let mut discard = Discard::new(Tile::new(TileId::new(0), kind));

        assert!(!discard.called());
        discard.mark_called();
        assert!(discard.called());
    }
}
