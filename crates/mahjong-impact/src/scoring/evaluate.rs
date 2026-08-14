//! 和牌判定与算点。
//!
//! 点数 = 底和 12 + 各番种点数之和。全交则不算点：胜者 400，其余三家归零。

use crate::config::{BASE_VALUE, ImpactRules};
use crate::hand::MeldKind;
use crate::scoring::shape::{self, KindCounts};
use crate::scoring::yaku::{AllInKind, Yaku, YakuValue};
use crate::tile::{Suit, TileKind};

/// 算点关心副露种类及其中每一张牌；吃牌的三张并不相同。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeldSummary {
    pub kind: MeldKind,
    pub tiles: Vec<TileKind>,
}

impl MeldSummary {
    #[must_use]
    pub fn new(kind: MeldKind, tile: TileKind) -> Self {
        Self {
            kind,
            tiles: vec![tile; usize::from(kind.tile_count())],
        }
    }

    #[must_use]
    pub fn from_tiles(kind: MeldKind, tiles: Vec<TileKind>) -> Self {
        Self { kind, tiles }
    }
}

/// 和牌时的全部输入。
#[derive(Clone, Copy, Debug)]
pub struct WinContext<'a> {
    pub rules: &'a ImpactRules,
    /// 本局的财神。
    pub joker: TileKind,
    /// 手牌，含和牌张与财神，不含副露。
    pub concealed: &'a [TileKind],
    pub melds: &'a [MeldSummary],
    /// 和的那一张。
    pub winning_tile: TileKind,
    /// 连庄次数（谁的连庄都算）。
    pub dealer_streak: u32,
    /// 和的是岭上牌。
    pub rinshan: bool,
    /// 加杠牌被抢和。
    pub chankan: bool,
    /// 和的是牌山最后一张。
    pub last_tile: bool,
    /// 天和 / 地和。
    pub blessing: bool,
    /// 和牌者当前的连打字牌数。
    pub honor_streak: u32,
}

/// 这手牌成立的牌型（可以同时成立多个）。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HandShapes {
    pub standard: bool,
    pub all_triplets: bool,
    pub seven_pairs: bool,
    pub thirteen_unrelated: bool,
    pub seven_gaps: bool,
    pub pao_long: bool,
}

impl HandShapes {
    #[must_use]
    pub const fn is_winning(self) -> bool {
        self.standard || self.seven_pairs || self.thirteen_unrelated || self.seven_gaps
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinEvaluation {
    shapes: HandShapes,
    yaku: Vec<YakuValue>,
    all_in: Option<AllInKind>,
    points: u32,
}

impl WinEvaluation {
    /// 三杠 / 连打十一风 / 四龙这类不经过牌型判定、直接触发的全交。
    #[must_use]
    pub fn from_trigger(kind: AllInKind) -> Self {
        Self {
            shapes: HandShapes::default(),
            yaku: vec![YakuValue::single(kind.fallback_yaku())],
            all_in: Some(kind),
            points: 0,
        }
    }

    #[must_use]
    pub const fn shapes(&self) -> HandShapes {
        self.shapes
    }

    #[must_use]
    pub fn yaku(&self) -> &[YakuValue] {
        &self.yaku
    }

    #[must_use]
    pub const fn all_in(&self) -> Option<AllInKind> {
        self.all_in
    }

    #[must_use]
    pub const fn is_all_in(&self) -> bool {
        self.all_in.is_some()
    }

    /// 和牌点数。全交时没有意义，恒为 0。
    #[must_use]
    pub const fn points(&self) -> u32 {
        self.points
    }

    /// 只给测试用：直接摆一个指定点数的普通和牌，不关心牌型。
    #[cfg(test)]
    pub(crate) fn for_test(points: u32) -> Self {
        Self {
            shapes: HandShapes::default(),
            yaku: Vec::new(),
            all_in: None,
            points,
        }
    }
}

/// 判和并算点。手牌不成型时返回 `None`。
#[must_use]
pub fn evaluate(context: &WinContext<'_>) -> Option<WinEvaluation> {
    if context.melds.len() > 4 {
        return None;
    }

    let (counts, concealed_jokers) = split(context.concealed, context.joker);
    let melded_jokers: u8 = context
        .melds
        .iter()
        .flat_map(|meld| meld.tiles.iter())
        .filter(|tile| **tile == context.joker)
        .count()
        .try_into()
        .expect("at most sixteen meld tiles");
    let jokers_total = concealed_jokers + melded_jokers;

    let melds_len = u8::try_from(context.melds.len()).expect("at most four melds");
    let kan_melds = u8::try_from(
        context
            .melds
            .iter()
            .filter(|meld| meld.kind.is_kan())
            .count(),
    )
    .expect("at most four melds");

    let shapes = detect_shapes(context, &counts, concealed_jokers, melds_len, kan_melds);
    if !shapes.is_winning() {
        return None;
    }

    // 「财神没有当作别的牌用」＝ 把财神并回它本来的牌种之后牌型依然成立。
    let mut natural_counts = counts;
    natural_counts[context.joker.index()] += concealed_jokers;
    let natural_shapes = detect_shapes(context, &natural_counts, 0, melds_len, kan_melds);

    let flush_suit = flush_suit(context);
    let jokers_stay_themselves = jokers_total == 0 || natural_shapes.is_winning();
    let pure_flush_no_joker = flush_suit.is_some_and(|suit| {
        jokers_total == 0 || (context.joker.suit() == Some(suit) && jokers_stay_themselves)
    });
    // 清七对要的是手里实打实摆出来的七个对子。一杠一达、二杠二达是靠杠顶掉两个对子
    // 再拿一张任意牌凑出来的，形状上算七对子，但不是清七对——有杠就不算。
    let pure_seven_pairs =
        kan_melds == 0 && shapes.seven_pairs && (jokers_total == 0 || natural_shapes.seven_pairs);

    let all_honors = all_honors(context);
    let single_wait = melds_len == 4 && context.concealed.len() == 2;
    let seven_winds = shapes.thirteen_unrelated && holds_every_honor_once(&counts);

    let conditions = [
        (
            AllInKind::ElevenHonorStreak,
            context.honor_streak >= 11,
            context.rules.all_in.eleven_honor_streak,
        ),
        (
            AllInKind::FourJokers,
            jokers_total >= 4,
            context.rules.all_in.four_jokers,
        ),
        (
            AllInKind::ThreeKans,
            kan_melds >= 3,
            context.rules.all_in.three_kans,
        ),
        (
            AllInKind::AllHonors,
            all_honors,
            context.rules.all_in.all_honors,
        ),
        (
            AllInKind::PureFlushNoJoker,
            pure_flush_no_joker,
            context.rules.all_in.pure_flush_no_joker,
        ),
        (
            AllInKind::PureSevenPairs,
            pure_seven_pairs,
            context.rules.all_in.pure_seven_pairs,
        ),
        (
            AllInKind::SingleWait,
            single_wait,
            context.rules.all_in.single_wait,
        ),
        (
            AllInKind::LastTile,
            context.last_tile,
            context.rules.all_in.last_tile,
        ),
        (
            AllInKind::Blessing,
            context.blessing,
            context.rules.all_in.blessing,
        ),
    ];

    if let Some(kind) = AllInKind::PRIORITY.into_iter().find(|kind| {
        conditions
            .iter()
            .any(|(candidate, holds, enabled)| candidate == kind && *holds && *enabled)
    }) {
        return Some(WinEvaluation {
            shapes,
            yaku: vec![YakuValue::single(kind.fallback_yaku())],
            all_in: Some(kind),
            points: 0,
        });
    }

    let mut yaku = Vec::new();

    match jokers_total {
        0 => yaku.push(YakuValue::single(Yaku::NoJoker)),
        2 => yaku.push(YakuValue::single(Yaku::TwoJokers)),
        3 => yaku.push(YakuValue::single(Yaku::ThreeJokers)),
        _ => {}
    }

    if shapes.seven_pairs {
        yaku.push(YakuValue::single(Yaku::SevenPairs));
    }
    if shapes.seven_gaps {
        yaku.push(YakuValue::single(Yaku::SevenGaps));
    }
    if shapes.all_triplets {
        yaku.push(YakuValue::single(Yaku::AllTriplets));
    }
    if shapes.thirteen_unrelated {
        yaku.push(YakuValue::single(Yaku::ThirteenUnrelated));
    }
    if seven_winds {
        yaku.push(YakuValue::single(Yaku::SevenWinds));
    }
    // 清一色：财神当了别的牌记「清一色」，否则记「无龙清一色」（后者是关掉全交后的降级项）。
    if flush_suit.is_some() && !pure_flush_no_joker {
        yaku.push(YakuValue::single(Yaku::PureFlush));
    }
    // 只在一杠一达、二杠二达这种「杠凑出来的七对子」上互斥：那张「任意牌」就是杠完摸
    // 上来的岭上牌，抛龙和杠开说的是同一张牌，只能算一个，算杠开。
    // 别的牌型不受影响——譬如 1s2s3s 4s5s6s 7s7s7s 8s8s8s + 财神杠掉 7s，抛龙靠的是
    // 单钓的那张财神，与岭上牌无关，两条照样一起算。
    let pao_long = shapes.pao_long && !(context.rinshan && shapes.seven_pairs && kan_melds > 0);
    if pao_long {
        yaku.push(YakuValue::single(Yaku::PaoLong));
    }
    if context.rinshan {
        yaku.push(YakuValue::single(Yaku::RinshanKaihou));
    }
    if context.chankan {
        yaku.push(YakuValue::single(Yaku::Chankan));
    }

    // 全交项关闭时，对应牌型改记 +10。单吊、无龙清一色的 +10 就是规则表里的那一条，不重复。
    for (kind, holds, enabled) in conditions {
        if holds && !enabled {
            yaku.push(YakuValue::single(kind.fallback_yaku()));
        }
    }

    if context.dealer_streak > 0 {
        yaku.push(YakuValue::new(Yaku::DealerStreak, context.dealer_streak));
    }

    let points = BASE_VALUE + yaku.iter().copied().map(YakuValue::points).sum::<u32>();

    Some(WinEvaluation {
        shapes,
        yaku,
        all_in: None,
        points,
    })
}

fn detect_shapes(
    context: &WinContext<'_>,
    counts: &KindCounts,
    jokers: u8,
    melds_len: u8,
    kan_melds: u8,
) -> HandShapes {
    let sets_needed = 4 - melds_len;
    let standard = shape::standard(counts, jokers, sets_needed, true, true);
    let all_triplets = context.melds.iter().all(|meld| meld.kind != MeldKind::Chi)
        && shape::standard(counts, jokers, sets_needed, true, false);

    // 一杠一达、二杠二达只容许真正的杠副露；只要还碰过（包括指示牌碰）或吃过，
    // 就不能再把杠折算成七对。逐组判断比比较计数更直接，也不会把按杠点结算但牌型
    // 仍是刻子的 IndicatorPon / IndicatorConcealed 误当成「杠达」。
    let only_kan_melds = context.melds.iter().all(|meld| meld.kind.is_kan());
    let seven_pairs = only_kan_melds
        && kan_melds <= 3
        && shape::pairs_shape(counts, jokers, 7 - 2 * kan_melds, kan_melds);

    let thirteen_unrelated = melds_len == 0 && shape::thirteen_unrelated(counts, jokers);
    let seven_gaps =
        context.rules.special.seven_gaps && melds_len == 0 && shape::seven_gaps(counts, jokers, 7);

    let pao_long = pao_long(context, counts, jokers, melds_len, kan_melds);

    HandShapes {
        standard,
        all_triplets,
        seven_pairs,
        thirteen_unrelated,
        seven_gaps,
        pao_long,
    }
}

/// 抛龙：和牌前 3+3+3+3 或 2+2+2+2+2+2 已成，多出的那一张财神单钓将，摸任意牌即和。
fn pao_long(
    context: &WinContext<'_>,
    counts: &KindCounts,
    jokers: u8,
    melds_len: u8,
    kan_melds: u8,
) -> bool {
    // 去掉和牌张，还原成 13 张（扣掉副露占的位置）。
    let (before_counts, before_jokers) = if context.winning_tile == context.joker {
        if jokers == 0 {
            return false;
        }
        (*counts, jokers - 1)
    } else {
        let index = context.winning_tile.index();
        if counts[index] == 0 {
            return false;
        }
        let mut before = *counts;
        before[index] -= 1;
        (before, jokers)
    };

    // 单钓将的那一张必须是财神。
    if before_jokers == 0 {
        return false;
    }
    let spare_jokers = before_jokers - 1;

    let sets = shape::standard(&before_counts, spare_jokers, 4 - melds_len, false, true);
    // 抛龙的七对分支与和牌形状使用同一条「副露只能有真杠」限制。
    let pairs = context.melds.iter().all(|meld| meld.kind.is_kan())
        && kan_melds <= 3
        && shape::pairs_shape(&before_counts, spare_jokers, 6 - 2 * kan_melds, kan_melds);

    sets || pairs
}

fn split(tiles: &[TileKind], joker: TileKind) -> (KindCounts, u8) {
    let mut counts = [0_u8; shape::KIND_COUNT];
    let mut jokers = 0;
    for kind in tiles {
        if *kind == joker {
            jokers += 1;
        } else {
            counts[kind.index()] += 1;
        }
    }
    (counts, jokers)
}

/// 手牌（含副露）里除财神以外全是同一门数牌时，返回那个花色。
fn flush_suit(context: &WinContext<'_>) -> Option<Suit> {
    let mut found: Option<Suit> = None;
    let mut any = false;

    for kind in non_joker_kinds(context) {
        let suit = kind.suit()?;
        any = true;
        match found {
            Some(current) if current != suit => return None,
            Some(_) => {}
            None => found = Some(suit),
        }
    }

    if any { found } else { None }
}

/// 除财神以外全是字牌。
fn all_honors(context: &WinContext<'_>) -> bool {
    let mut any = false;
    for kind in non_joker_kinds(context) {
        if !kind.is_honor() {
            return false;
        }
        any = true;
    }
    any
}

fn non_joker_kinds<'a>(context: &'a WinContext<'a>) -> impl Iterator<Item = TileKind> + 'a {
    let joker = context.joker;
    context
        .concealed
        .iter()
        .copied()
        .chain(
            context
                .melds
                .iter()
                .flat_map(|meld| meld.tiles.iter().copied()),
        )
        .filter(move |kind| *kind != joker)
}

/// 七风齐：东南西北中发白在非财神的牌里恰好各一张。
fn holds_every_honor_once(counts: &KindCounts) -> bool {
    counts[27..34].iter().all(|count| *count == 1)
}

#[cfg(test)]
mod tests {
    use super::{MeldSummary, WinContext, WinEvaluation, evaluate};
    use crate::config::ImpactRules;
    use crate::hand::MeldKind;
    use crate::scoring::yaku::{AllInKind, Yaku};
    use crate::tile::TileKind;

    fn kind(code: &str) -> TileKind {
        code.parse().expect("valid tile code")
    }

    fn kinds(spec: &str) -> Vec<TileKind> {
        spec.split_whitespace().map(kind).collect()
    }

    struct Case {
        rules: ImpactRules,
        joker: TileKind,
        concealed: Vec<TileKind>,
        melds: Vec<MeldSummary>,
        winning_tile: TileKind,
        dealer_streak: u32,
        rinshan: bool,
        chankan: bool,
        last_tile: bool,
        blessing: bool,
        honor_streak: u32,
    }

    impl Case {
        fn new(joker: &str, concealed: &str, winning_tile: &str) -> Self {
            Self {
                rules: ImpactRules::standard(),
                joker: kind(joker),
                concealed: kinds(concealed),
                melds: Vec::new(),
                winning_tile: kind(winning_tile),
                dealer_streak: 0,
                rinshan: false,
                chankan: false,
                last_tile: false,
                blessing: false,
                honor_streak: 0,
            }
        }

        fn evaluate(&self) -> Option<WinEvaluation> {
            evaluate(&WinContext {
                rules: &self.rules,
                joker: self.joker,
                concealed: &self.concealed,
                melds: &self.melds,
                winning_tile: self.winning_tile,
                dealer_streak: self.dealer_streak,
                rinshan: self.rinshan,
                chankan: self.chankan,
                last_tile: self.last_tile,
                blessing: self.blessing,
                honor_streak: self.honor_streak,
            })
        }

        fn win(&self) -> WinEvaluation {
            self.evaluate().expect("hand should be complete")
        }
    }

    fn yaku_names(evaluation: &WinEvaluation) -> Vec<&'static str> {
        evaluation
            .yaku()
            .iter()
            .map(|value| value.yaku().as_str())
            .collect()
    }

    #[test]
    fn plain_hand_scores_the_base_value_plus_no_joker() {
        let case = Case::new("5m", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
        let win = case.win();

        assert!(win.shapes().standard);
        assert_eq!(yaku_names(&win), ["no_joker"]);
        assert_eq!(win.points(), 13);
        assert!(!win.is_all_in());
    }

    #[test]
    fn incomplete_hands_do_not_win() {
        let case = Case::new("5m", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 2z 3z 4z", "4z");

        assert!(case.evaluate().is_none());
    }

    #[test]
    fn an_open_sequence_is_scored_as_a_sequence_not_a_triplet() {
        let mut case = Case::new("5m", "4p 4p 4p 7s 7s 7s 1z 1z 1z 2z 2z", "2z");
        case.melds = vec![MeldSummary::from_tiles(MeldKind::Chi, kinds("1m 2m 3m"))];

        let win = case.win();
        assert!(win.shapes().standard);
        assert!(!win.shapes().all_triplets);
    }

    #[test]
    fn seven_pairs_with_substituting_jokers_is_not_pure() {
        let case = Case::new("5m", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 9s 5m 5m", "9s");
        let win = case.win();

        assert!(win.shapes().seven_pairs);
        assert!(!win.is_all_in(), "财神当了别的牌就不是清七对");
        // 和牌前是 5 对 +「4s 加一张财神」+ 一张财神单钓，摸任意牌都能和，所以也是抛龙。
        assert_eq!(yaku_names(&win), ["two_jokers", "seven_pairs", "pao_long"]);
        assert_eq!(win.points(), 12 + 1 + 1 + 10);
    }

    #[test]
    fn all_honors_wins_by_all_in_and_reports_only_that_yaku() {
        let case = Case::new("1m", "1z 1z 2z 2z 3z 3z 4z 4z 5z 5z 6z 6z 7z 7z", "7z");
        let win = case.win();

        assert_eq!(win.all_in(), Some(AllInKind::AllHonors));
        assert_eq!(yaku_names(&win), ["all_honors"]);
        assert_eq!(win.points(), 0);
    }

    #[test]
    fn a_disabled_all_in_falls_back_to_ten_points() {
        let mut case = Case::new("1m", "1z 1z 2z 2z 3z 3z 4z 4z 5z 5z 6z 6z 7z 7z", "7z");
        case.rules.all_in.all_honors = false;
        case.rules.all_in.pure_seven_pairs = false;
        let win = case.win();

        assert!(!win.is_all_in());
        assert_eq!(
            yaku_names(&win),
            ["no_joker", "seven_pairs", "all_honors", "pure_seven_pairs"]
        );
        assert_eq!(win.points(), 12 + 1 + 1 + 10 + 10);
    }

    #[test]
    fn pure_flush_without_a_substituting_joker_is_an_all_in() {
        let case = Case::new("1z", "1m 2m 3m 4m 5m 6m 7m 8m 9m 1m 2m 3m 5m 5m", "5m");
        let win = case.win();

        assert_eq!(win.all_in(), Some(AllInKind::PureFlushNoJoker));
    }

    #[test]
    fn pure_flush_with_a_substituting_joker_scores_ten() {
        let case = Case::new("1z", "1m 2m 3m 4m 5m 6m 7m 8m 9m 1m 2m 5m 5m 1z", "5m");
        let win = case.win();

        assert!(!win.is_all_in(), "财神顶了一张万子就只是清一色");
        assert!(yaku_names(&win).contains(&"pure_flush"));
        assert_eq!(win.points(), 12 + 10);
    }

    #[test]
    fn pao_long_scores_ten_on_any_winning_tile() {
        let case = Case::new("5s", "1m 2m 3m 4m 5m 6m 7m 8m 9m 1p 2p 3p 5s 9s", "9s");
        let win = case.win();

        assert!(win.shapes().pao_long);
        assert_eq!(yaku_names(&win), ["pao_long"]);
        assert_eq!(win.points(), 12 + 10);
    }

    #[test]
    fn seven_winds_rides_on_thirteen_unrelated() {
        let case = Case::new("9s", "1z 2z 3z 4z 5z 6z 7z 1m 4m 7m 1p 4p 7p 1s", "1s");
        let win = case.win();

        assert!(win.shapes().thirteen_unrelated);
        assert_eq!(
            yaku_names(&win),
            ["no_joker", "thirteen_unrelated", "seven_winds"]
        );
        assert_eq!(win.points(), 12 + 3);
    }

    #[test]
    fn single_wait_needs_four_melds_and_one_pair() {
        let mut case = Case::new("5z", "3s 3s", "3s");
        case.melds = vec![
            MeldSummary::new(MeldKind::Pon, kind("1m")),
            MeldSummary::new(MeldKind::Pon, kind("2p")),
            MeldSummary::new(MeldKind::Pon, kind("3p")),
            MeldSummary::new(MeldKind::Pon, kind("7z")),
        ];

        assert_eq!(case.win().all_in(), Some(AllInKind::SingleWait));

        case.rules.all_in.single_wait = false;
        let win = case.win();
        assert_eq!(
            yaku_names(&win),
            ["no_joker", "all_triplets", "single_wait"]
        );
        assert_eq!(win.points(), 12 + 1 + 1 + 10);
    }

    #[test]
    fn indicator_melds_never_count_toward_three_kans() {
        let mut case = Case::new("5z", "3s 3s", "3s");
        case.melds = vec![
            MeldSummary::new(MeldKind::IndicatorPon, kind("1m")),
            MeldSummary::new(MeldKind::IndicatorConcealed, kind("2p")),
            MeldSummary::new(MeldKind::IndicatorPon, kind("3p")),
            MeldSummary::new(MeldKind::Pon, kind("7z")),
        ];
        case.rules.all_in.single_wait = false;
        let win = case.win();

        assert!(!yaku_names(&win).contains(&"three_kans"));
    }

    #[test]
    fn every_dealer_streak_adds_ten_points() {
        let mut case = Case::new("5m", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
        case.dealer_streak = 3;
        let win = case.win();

        assert_eq!(yaku_names(&win), ["no_joker", "dealer_streak"]);
        assert_eq!(win.points(), 12 + 1 + 30);
    }

    #[test]
    fn rinshan_adds_ten_points() {
        let mut case = Case::new("5m", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
        case.rinshan = true;

        assert_eq!(case.win().points(), 12 + 1 + 10);
    }

    #[test]
    fn seven_pairs_counts_a_kan_as_two_pairs() {
        let mut case = Case::new("9s", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s", "4s");
        case.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("1z"))];
        let win = case.win();

        assert!(
            win.shapes().seven_pairs,
            "一杠一达：1 杠 + 5 对 + 1 张任意牌"
        );
    }

    #[test]
    fn kan_reached_seven_pairs_rejects_any_pon_meld() {
        let mut one_kan = Case::new("5z", "1m 2m 3m 4p 5p 6p 7s 7s", "7s");
        one_kan.melds = vec![
            MeldSummary::new(MeldKind::ConcealedKan, kind("1z")),
            MeldSummary::new(MeldKind::Pon, kind("2z")),
        ];
        let win = one_kan.win();
        assert!(win.shapes().standard, "这手仍是普通和牌");
        assert!(
            !win.shapes().seven_pairs,
            "一杠之后有普通碰牌，不能算一杠一达"
        );

        let mut two_kans = Case::new("5z", "1m 2m 3m 7s 7s", "7s");
        two_kans.melds = vec![
            MeldSummary::new(MeldKind::OpenKan, kind("1z")),
            MeldSummary::new(MeldKind::AddedKan, kind("2z")),
            MeldSummary::new(MeldKind::IndicatorPon, kind("3z")),
        ];
        let win = two_kans.win();
        assert!(win.shapes().standard, "这手仍是普通和牌");
        assert!(
            !win.shapes().seven_pairs,
            "二杠之外有指示牌碰，不能算二杠二达"
        );
    }

    #[test]
    fn a_kan_keeps_seven_pairs_out_of_the_pure_seven_pairs_all_in() {
        // 同一手牌：没有杠的时候是清七对全交，用杠顶掉两个对子就不是了。
        let pure = Case::new("9s", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 6s 6s", "6s");
        assert_eq!(pure.win().all_in(), Some(AllInKind::PureSevenPairs));

        let mut kanned = Case::new("9s", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s", "4s");
        kanned.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("1z"))];
        let win = kanned.win();

        assert!(win.shapes().seven_pairs);
        assert!(!win.is_all_in(), "有杠就不算清七对");
        assert_eq!(yaku_names(&win), ["no_joker", "seven_pairs"]);
        assert_eq!(win.points(), 12 + 1 + 1);
    }

    #[test]
    fn a_kan_hand_scores_the_replacement_tile_as_rinshan_instead_of_pao_long() {
        // 一杠一达：1 杠 + 4 对 +「9s 加一张财神」+ 一张财神单钓，形状上抛龙成立。
        let concealed = "1m 1m 3m 3m 5p 5p 7p 7p 9s 5m 5m";
        let mut case = Case::new("5m", concealed, "9s");
        case.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("1z"))];

        // 不是岭上牌和的，抛龙照算。
        let win = case.win();
        assert!(win.shapes().pao_long);
        assert_eq!(yaku_names(&win), ["two_jokers", "seven_pairs", "pao_long"]);

        // 和的是杠完摸上来的那张——抛龙的「任意牌」和杠开是同一张，只算杠开。
        case.rinshan = true;
        let win = case.win();
        let names = yaku_names(&win);
        assert!(!names.contains(&"pao_long"), "杠开优先，不再叠抛龙");
        assert_eq!(names, ["two_jokers", "seven_pairs", "rinshan_kaihou"]);
        assert_eq!(win.points(), 12 + 1 + 1 + 10);
    }

    #[test]
    fn a_kan_and_five_pairs_wins_on_anything_without_being_pao_long() {
        // 一杠一达做完之后手上剩 5 对，那张任意牌就是刚摸的这一张——摸什么都能和。
        for winning in ["4s", "9p", "3z", "1m"] {
            let concealed = format!("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s {winning}");
            let mut case = Case::new("9s", &concealed, winning);
            case.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("1z"))];
            case.rinshan = true;
            let win = case.win();

            assert!(win.shapes().seven_pairs, "{winning} 应当成和");
            // 摸什么都能和是那张「任意牌」给的，不是财神单钓——手上一张财神都没有。
            assert!(!win.shapes().pao_long, "没有财神就没有龙可抛");
            let names = yaku_names(&win);
            assert!(!names.contains(&"pao_long"));
            assert!(names.contains(&"rinshan_kaihou"), "岭上牌和的算杠上开花");
        }
    }

    #[test]
    fn a_standard_shape_still_stacks_pao_long_on_top_of_rinshan() {
        // 1s2s3s 4s5s6s 7s7s7s 8s8s8s + 财神：杠掉 7s，岭上摸什么都能和。
        // 这里抛龙靠的是那张单钓的财神，跟岭上牌是两码事，两条都算。
        let mut case = Case::new("1z", "1s 2s 3s 4s 5s 6s 8s 8s 8s 1z 9s", "9s");
        case.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("7s"))];
        case.rinshan = true;
        let win = case.win();

        assert!(win.shapes().standard);
        assert!(!win.shapes().seven_pairs, "这不是七对子牌型");
        assert_eq!(
            yaku_names(&win),
            ["pure_flush", "pao_long", "rinshan_kaihou"]
        );
        assert_eq!(win.points(), 12 + 10 + 10 + 10);
    }

    #[test]
    fn seven_pairs_accepts_duplicate_pairs() {
        // 四张一样的牌算两个对子，重复的对子照收。
        let case = Case::new("9s", "1m 1m 1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s", "4s");

        assert!(case.win().shapes().seven_pairs);
    }

    #[test]
    fn triggers_can_be_reported_without_a_hand_shape() {
        let win = WinEvaluation::from_trigger(AllInKind::ThreeKans);

        assert_eq!(win.all_in(), Some(AllInKind::ThreeKans));
        assert_eq!(win.yaku()[0].yaku(), Yaku::ThreeKans);
        assert_eq!(win.points(), 0);
    }
}
