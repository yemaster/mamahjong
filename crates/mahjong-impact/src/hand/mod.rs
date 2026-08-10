//! 单局（一局牌）的状态机与值对象。

mod model;
mod state;

pub use model::{
    Discard, DrawSource, EndReason, HandError, HandPhase, Meld, MeldId, MeldKind, Reaction,
    ReactionKind,
};
pub use state::{HandOutcome, ImpactHand, PlayerHand, ReactionOptions, TurnAction, TurnActions};
