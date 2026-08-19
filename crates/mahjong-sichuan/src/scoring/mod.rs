//! 四川麻将的番型识别与计算。

mod evaluate;
mod shape;
mod yaku;

pub use evaluate::{
    HandShapes, MAX_FAN, MeldSummary, WinContext, WinEvaluation, evaluate, score_for,
};
pub use yaku::{Yaku, YakuValue};
