//! 番型计算。
//!
//! 输入一手已经「牌型成立」的牌（副露 + 暗牌 + 得牌方式），输出番数、分数与番种列表。
//! 基础番型取最高、加番叠加；封顶见 [`MAX_FAN`]。

use crate::hand::MeldKind;
use crate::scoring::shape::{self, KindCounts};
use crate::scoring::yaku::{Yaku, YakuValue};
use crate::tile::{Suit, TileKind};

/// 封顶番数：6 番，对应 32000 分。
pub const MAX_FAN: u32 = 6;

/// 一倍的分值：川麻以 1000 分为一倍。
pub const BASE_POINTS: u32 = 1000;

/// 一个副露面子的摘要，供算番使用。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeldSummary {
    kind: MeldKind,
    tiles: Vec<TileKind>,
}

impl MeldSummary {
    #[must_use]
    pub fn new(kind: MeldKind, tiles: Vec<TileKind>) -> Self {
        Self { kind, tiles }
    }

    #[must_use]
    pub const fn kind(&self) -> MeldKind {
        self.kind
    }

    #[must_use]
    pub fn tiles(&self) -> &[TileKind] {
        &self.tiles
    }
}

/// 一次和牌的输入：牌型之外还要知道怎么胡的，好把加番算进去。
#[derive(Clone, Debug)]
pub struct WinContext<'a> {
    /// 暗牌（含得牌那张）。
    pub concealed: &'a [TileKind],
    pub melds: &'a [MeldSummary],
    /// 自摸。
    pub is_tsumo: bool,
    /// 杠上花。
    pub rinshan: bool,
    /// 杠上炮。
    pub gang_pao: bool,
    /// 抢杠胡。
    pub chankan: bool,
    /// 海底（摸到最后一张胡牌）。
    pub is_last_tile: bool,
    /// 天胡 / 地胡。
    pub blessing: bool,
}

/// 和牌时成立的牌型，三种互有重叠（对对胡也是标准型）。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandShapes {
    standard: bool,
    all_triplets: bool,
    seven_pairs: bool,
}

impl HandShapes {
    #[must_use]
    pub const fn is_winning(self) -> bool {
        self.standard || self.seven_pairs
    }

    #[must_use]
    pub const fn all_triplets(self) -> bool {
        self.all_triplets
    }

    #[must_use]
    pub const fn seven_pairs(self) -> bool {
        self.seven_pairs
    }
}

/// 一次和牌的完整结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinEvaluation {
    fan: u32,
    score: u32,
    yaku: Vec<YakuValue>,
    shapes: HandShapes,
}

impl WinEvaluation {
    #[must_use]
    pub const fn fan(&self) -> u32 {
        self.fan
    }

    #[must_use]
    pub const fn score(&self) -> u32 {
        self.score
    }

    #[must_use]
    pub fn yaku(&self) -> &[YakuValue] {
        &self.yaku
    }

    #[must_use]
    pub const fn shapes(&self) -> HandShapes {
        self.shapes
    }

    /// 直接给一套结果，测试用。
    #[must_use]
    pub fn for_test(fan: u32, score: u32, yaku: Vec<YakuValue>) -> Self {
        Self {
            fan,
            score,
            yaku,
            shapes: HandShapes {
                standard: true,
                all_triplets: false,
                seven_pairs: false,
            },
        }
    }
}

/// 计算和牌番数。牌型不成立返回 `None`。
#[must_use]
pub fn evaluate(context: &WinContext<'_>) -> Option<WinEvaluation> {
    if context.melds.len() > 4 {
        return None;
    }

    let counts = counts(context.concealed);
    let melds_len = u8::try_from(context.melds.len()).expect("checked to be at most 4");
    let shapes = detect_shapes(&counts, melds_len);
    if !shapes.is_winning() {
        return None;
    }

    let flush = flush_suit(context).is_some();
    let concealed_gen = concealed_gen(&counts);
    let kan_melds = u32::try_from(
        context
            .melds
            .iter()
            .filter(|meld| meld.kind().is_kan())
            .count(),
    )
    .expect("at most four melds");

    let mut yaku = Vec::new();
    let mut fan = 0_u32;
    push(
        &mut yaku,
        &mut fan,
        YakuValue::single(base_yaku(shapes, flush, concealed_gen, context.blessing)),
    );
    if context.is_tsumo {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::ZiMo));
    }
    if kan_melds > 0 {
        push(&mut yaku, &mut fan, YakuValue::new(Yaku::Gen, kan_melds));
    }
    if context.rinshan {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::GangShangHua));
    }
    if context.gang_pao {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::GangShangPao));
    }
    if context.chankan {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::QiangGangHu));
    }
    if gold_hook(context) {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::JinGouDiao));
    }
    if context.is_last_tile {
        push(&mut yaku, &mut fan, YakuValue::single(Yaku::HaiDi));
    }

    let score = score_for(fan);
    Some(WinEvaluation {
        fan,
        score,
        yaku,
        shapes,
    })
}

/// 番数 → 分数：1000 × 2^(番−1)，封顶 6 番。
#[must_use]
pub const fn score_for(fan: u32) -> u32 {
    let capped = if fan > MAX_FAN { MAX_FAN } else { fan };
    BASE_POINTS << capped.saturating_sub(1)
}

fn push(yaku: &mut Vec<YakuValue>, fan: &mut u32, value: YakuValue) {
    *fan += value.fan();
    yaku.push(value);
}

fn counts(concealed: &[TileKind]) -> KindCounts {
    let mut counts = [0_u8; shape::KIND_COUNT];
    for kind in concealed {
        counts[kind.index()] += 1;
    }
    counts
}

fn detect_shapes(counts: &KindCounts, melds_len: u8) -> HandShapes {
    let sets_needed = 4 - melds_len;
    let standard = shape::standard(counts, 0, sets_needed, true, true);
    let all_triplets = shape::standard(counts, 0, sets_needed, true, false);
    let seven_pairs = melds_len == 0 && shape::pairs_shape(counts, 0, 7, 0);
    HandShapes {
        standard,
        all_triplets,
        seven_pairs,
    }
}

/// 全部牌（暗牌 + 副露）是不是同一门花色；是则返回那一门。
fn flush_suit(context: &WinContext<'_>) -> Option<Suit> {
    let mut found = None;
    for kind in context.concealed.iter().copied().chain(
        context
            .melds
            .iter()
            .flat_map(|meld| meld.tiles().iter().copied()),
    ) {
        let suit = kind.suit()?;
        match found {
            None => found = Some(suit),
            Some(current) if current != suit => return None,
            Some(_) => {}
        }
    }
    found
}

/// 暗牌里有几个「四张相同」的种（龙七对判定用）。
fn concealed_gen(counts: &KindCounts) -> u8 {
    u8::try_from(counts.iter().filter(|count| **count == 4).count()).expect("at most 34 kinds")
}

/// 金钩钓：四副副露、单钓将。
fn gold_hook(context: &WinContext<'_>) -> bool {
    context.melds.len() == 4 && context.concealed.len() == 2
}

/// 基础番型取最高、不叠加。
fn base_yaku(shapes: HandShapes, flush: bool, concealed_gen: u8, blessing: bool) -> Yaku {
    if blessing {
        Yaku::TianHuDiHu
    } else if shapes.seven_pairs() {
        if flush {
            Yaku::QingQiDui
        } else if concealed_gen >= 1 {
            Yaku::LongQiDui
        } else {
            Yaku::QiDui
        }
    } else if shapes.all_triplets() {
        if flush { Yaku::QingDui } else { Yaku::DuiDuiHu }
    } else if flush {
        Yaku::QingYiSe
    } else {
        Yaku::PingHu
    }
}

#[cfg(test)]
mod tests {
    use super::{MeldSummary, WinContext, evaluate, score_for};
    use crate::hand::MeldKind;
    use crate::scoring::yaku::Yaku;
    use crate::tile::TileKind;

    fn kinds(spec: &str) -> Vec<TileKind> {
        spec.split_whitespace()
            .map(|code| code.parse().expect("valid tile code"))
            .collect()
    }

    fn context<'a>(
        concealed: &'a [TileKind],
        melds: &'a [MeldSummary],
        tsumo: bool,
    ) -> WinContext<'a> {
        WinContext {
            concealed,
            melds,
            is_tsumo: tsumo,
            rinshan: false,
            gang_pao: false,
            chankan: false,
            is_last_tile: false,
            blessing: false,
        }
    }

    #[test]
    fn a_plain_mixed_hand_is_ping_hu_at_one_fan() {
        let concealed = kinds("1m 2m 3m 4p 5p 6p 7s 8s 9s 1m 1m 1m 2p 2p");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 1);
        assert_eq!(result.score(), 1000);
        assert_eq!(result.yaku().first().map(|y| y.yaku()), Some(Yaku::PingHu));
    }

    #[test]
    fn tsumo_adds_one_fan() {
        let concealed = kinds("1m 2m 3m 4p 5p 6p 7s 8s 9s 1m 1m 1m 2p 2p");
        let result = evaluate(&context(&concealed, &[], true)).expect("winning hand");

        assert_eq!(result.fan(), 2);
        assert_eq!(result.score(), 2000);
        assert!(result.yaku().iter().any(|y| y.yaku() == Yaku::ZiMo));
    }

    #[test]
    fn a_flush_is_qing_yi_se_at_three_fan() {
        let concealed = kinds("1m 2m 3m 4m 5m 6m 7m 8m 9m 1m 1m 1m 2m 2m");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 3);
        assert_eq!(
            result.yaku().first().map(|y| y.yaku()),
            Some(Yaku::QingYiSe)
        );
    }

    #[test]
    fn all_triplets_is_dui_dui_hu() {
        let concealed = kinds("1m 1m 1m 4p 4p 4p 7s 7s 7s 2m 2m 2m 3p 3p");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 2);
        assert_eq!(
            result.yaku().first().map(|y| y.yaku()),
            Some(Yaku::DuiDuiHu)
        );
    }

    #[test]
    fn flush_all_triplets_is_qing_dui() {
        let concealed = kinds("1m 1m 1m 2m 2m 2m 3m 3m 3m 4m 4m 4m 5m 5m");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 4);
        assert_eq!(result.yaku().first().map(|y| y.yaku()), Some(Yaku::QingDui));
    }

    #[test]
    fn seven_pairs_is_qi_dui() {
        let concealed = kinds("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 6s 6s");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 3);
        assert_eq!(result.yaku().first().map(|y| y.yaku()), Some(Yaku::QiDui));
    }

    #[test]
    fn seven_pairs_with_a_quad_is_long_qi_dui() {
        let concealed = kinds("1m 1m 1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s");
        let result = evaluate(&context(&concealed, &[], false)).expect("winning hand");

        assert_eq!(result.fan(), 5);
        assert_eq!(
            result.yaku().first().map(|y| y.yaku()),
            Some(Yaku::LongQiDui)
        );
    }

    #[test]
    fn a_concealed_kan_counts_as_one_gen() {
        let melds = vec![MeldSummary::new(
            MeldKind::ConcealedKan,
            kinds("9s 9s 9s 9s"),
        )];
        let concealed = kinds("1m 2m 3m 4p 5p 6p 1s 2s 3s 2m 2m");
        let result = evaluate(&context(&concealed, &melds, false)).expect("winning hand");

        // 平胡 1 + 根 1 = 2 番。
        assert_eq!(result.fan(), 2);
        assert!(result.yaku().iter().any(|y| y.yaku() == Yaku::Gen));
    }

    #[test]
    fn a_blessing_hand_is_tian_hu_di_hu() {
        let concealed = kinds("1m 2m 3m 4p 5p 6p 7s 8s 9s 1m 1m 1m 2p 2p");
        let mut context = context(&concealed, &[], false);
        context.blessing = true;
        let result = evaluate(&context).expect("winning hand");

        // 天胡 / 地胡 6 番封顶。
        assert_eq!(result.fan(), 6);
        assert_eq!(
            result.yaku().first().map(|y| y.yaku()),
            Some(Yaku::TianHuDiHu)
        );
    }

    #[test]
    fn the_score_caps_at_six_fan() {
        assert_eq!(score_for(1), 1000);
        assert_eq!(score_for(2), 2000);
        assert_eq!(score_for(3), 4000);
        assert_eq!(score_for(6), 32000);
        assert_eq!(score_for(7), 32000, "7 番封顶到 6 番");
        assert_eq!(score_for(20), 32000);
    }

    #[test]
    fn too_many_melds_is_not_a_winning_hand() {
        let melds: Vec<MeldSummary> = (0..5)
            .map(|_| MeldSummary::new(MeldKind::Pon, kinds("1m 1m 1m")))
            .collect();
        let concealed = kinds("2p 2p");

        assert!(evaluate(&context(&concealed, &melds, false)).is_none());
    }
}
