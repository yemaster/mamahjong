//! 单局（一局牌）的状态机与值对象。

mod model;
mod state;

pub use model::{
    Discard, DrawSource, EndReason, HandError, HandPhase, Meld, MeldId, MeldKind, Reaction,
    ReactionKind,
};
pub use state::{
    KanEvent, PlayerHand, ReactionOptions, SichuanHand, TurnAction, TurnActions, WinnerRecord,
};
