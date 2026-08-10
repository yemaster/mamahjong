//! 冲击麻将规则的对外行为验收：财神推导、牌山开门、牌型判定、
//! 每个全交开关的开 / 关两条路径，以及单节结算。
//!
//! 这里只用公开 API，等于从上层（应用层 / 服务端）的角度把规则跑一遍。

use mahjong_impact::{
    AllInKind, ImpactMatch, ImpactRules, MatchError, MeldKind, MeldSummary, Seat, TileKind, Wall,
    WallSeed, WinContext, WinEvaluation, Yaku, evaluate, joker_of,
};

fn kind(code: &str) -> TileKind {
    code.parse().expect("valid tile code")
}

fn kinds(spec: &str) -> Vec<TileKind> {
    spec.split_whitespace().map(kind).collect()
}

fn seat(index: u8) -> Seat {
    Seat::new(index).expect("valid seat")
}

/// 一次和牌判定的输入。默认全是「什么特殊局面都没有」。
struct Win {
    rules: ImpactRules,
    joker: TileKind,
    concealed: Vec<TileKind>,
    melds: Vec<MeldSummary>,
    winning_tile: TileKind,
    dealer_streak: u32,
    rinshan: bool,
    last_tile: bool,
    blessing: bool,
    honor_streak: u32,
}

impl Win {
    fn new(joker: &str, concealed: &str, winning_tile: &str) -> Self {
        Self {
            rules: ImpactRules::standard(),
            joker: kind(joker),
            concealed: kinds(concealed),
            melds: Vec::new(),
            winning_tile: kind(winning_tile),
            dealer_streak: 0,
            rinshan: false,
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

// ---- 财神推导 ----

#[test]
fn the_joker_follows_the_indicator_around_every_cycle() {
    assert_eq!(joker_of(kind("9m")), kind("1m"), "数牌 9 回到 1");
    assert_eq!(joker_of(kind("8p")), kind("9p"));
    assert_eq!(joker_of(kind("4z")), kind("1z"), "北回到东");
    assert_eq!(joker_of(kind("3z")), kind("4z"), "西到北");
    assert_eq!(joker_of(kind("5z")), kind("7z"), "白回到中");
    assert_eq!(joker_of(kind("7z")), kind("6z"), "中到发");
    assert_eq!(joker_of(kind("6z")), kind("5z"), "发到白");
}

// ---- 牌山与开门 ----

#[test]
fn the_wall_hands_out_every_tile_but_the_indicator_stack() {
    let wall = Wall::new(seat(0), &WallSeed::from_bytes([23; 32]));

    assert_eq!(wall.remaining_draws(), 134, "136 张里翻财神那一墩不摸");
    assert_eq!(wall.joker(), joker_of(wall.indicator().kind()));

    let dice = wall.dice();
    assert!((1..=6).contains(&dice.first()) && (1..=6).contains(&dice.second()));
    assert_eq!(dice.sum(), dice.first() + dice.second());
    assert_eq!(
        wall.break_seat(),
        seat(0).offset_by((dice.sum() - 1) % Seat::COUNT),
        "从庄家数 1、逆时针数到骰子点数"
    );
}

#[test]
fn the_indicator_stack_never_comes_out_of_the_wall() {
    let mut wall = Wall::new(seat(1), &WallSeed::from_bytes([77; 32]));
    let dead: Vec<_> = wall
        .dead_positions()
        .into_iter()
        .map(|position| wall.ordered_tiles()[position].id())
        .collect();

    let mut drawn = Vec::with_capacity(134);
    while let Some(tile) = wall.draw() {
        drawn.push(tile);
    }

    assert_eq!(drawn.len(), 134);
    assert!(
        !drawn.iter().any(|tile| dead.contains(&tile.id())),
        "翻开的指示牌和它下面那张都摸不到"
    );
}

// ---- 牌型 ----

#[test]
fn a_kan_counts_as_two_pairs_in_seven_pairs() {
    let mut one_kan = Win::new("5z", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 9s", "9s");
    one_kan.melds = vec![MeldSummary::new(MeldKind::ConcealedKan, kind("1z"))];
    assert!(
        one_kan.win().shapes().seven_pairs,
        "一杠一达：1 杠 + 5 对 + 1 张"
    );

    let mut two_kans = Win::new("5z", "1m 1m 3m 3m 5p 5p 7s 9s", "9s");
    two_kans.melds = vec![
        MeldSummary::new(MeldKind::ConcealedKan, kind("1z")),
        MeldSummary::new(MeldKind::ConcealedKan, kind("2z")),
    ];
    assert!(
        two_kans.win().shapes().seven_pairs,
        "二杠二达：2 杠 + 3 对 + 2 张"
    );
}

#[test]
fn thirteen_unrelated_needs_every_pair_of_same_suit_tiles_more_than_two_apart() {
    let good = Win::new("9s", "1m 4m 7m 1p 4p 7p 1s 4s 1z 2z 3z 4z 5z 6z", "6z");
    assert!(good.win().shapes().thirteen_unrelated);

    // 4s 与 5s 只差 1，不成十三不搭，也凑不出别的牌型。
    let bad = Win::new("9s", "1m 4m 7m 1p 4p 7p 1s 4s 5s 1z 2z 3z 4z", "5z");
    assert!(bad.evaluate().is_none());
}

#[test]
fn seven_winds_needs_all_seven_honors_exactly_once() {
    let case = Win::new("9s", "1z 2z 3z 4z 5z 6z 7z 1m 4m 7m 1p 4p 7p 1s", "1s");
    let win = case.win();

    assert!(win.shapes().thirteen_unrelated);
    assert_eq!(
        yaku_names(&win),
        ["no_joker", "thirteen_unrelated", "seven_winds"]
    );
    assert_eq!(win.points(), 12 + 1 + 1 + 1);
}

#[test]
fn seven_gaps_only_exists_when_the_special_rule_is_switched_on() {
    let mut case = Win::new("5z", "1m 3m 5m 7m 2m 4m 2p 4p 6p 8p 1s 3s 5s 7s", "7s");
    assert!(case.evaluate().is_none(), "七嵌默认关闭");

    case.rules.special.seven_gaps = true;
    let win = case.win();
    assert!(win.shapes().seven_gaps);
    assert_eq!(yaku_names(&win), ["no_joker", "seven_gaps"]);
    assert_eq!(win.points(), 12 + 1 + 1);
}

#[test]
fn pao_long_wins_on_any_tile_at_all() {
    // 四组已成 + 一张财神单钓将，和牌张随便是什么。
    for tail in ["9s", "3z", "5p"] {
        let case = Win::new(
            "5s",
            &format!("1m 2m 3m 4m 5m 6m 7m 8m 9m 1p 2p 3p 5s {tail}"),
            tail,
        );
        let win = case.win();

        assert!(win.shapes().pao_long, "抛龙摸 {tail} 也和");
        assert_eq!(win.points(), 12 + 10);
    }
}

#[test]
fn a_jokerless_hand_and_a_three_joker_hand_score_differently() {
    let none = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    assert_eq!(yaku_names(&none.win()), ["no_joker"]);
    assert_eq!(none.win().points(), 12 + 1);

    let two = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 5z 5z 1z 1z 2z 2z", "2z");
    assert_eq!(yaku_names(&two.win()), ["two_jokers"]);
    assert_eq!(two.win().points(), 12 + 1);

    // 三张财神几乎必然同时成立抛龙（多出的那张财神本来就能单钓将），所以只查番种在不在。
    let three = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 5z 5z 5z 1z 1z", "1z");
    assert!(yaku_names(&three.win()).contains(&"three_jokers"));
    assert!(three.win().points() >= 12 + 2, "三财神值 2 点");
}

#[test]
fn every_dealer_streak_is_worth_ten_points_no_matter_whose_it_is() {
    let mut case = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    case.dealer_streak = 4;

    assert_eq!(case.win().points(), 12 + 1 + 40);
}

// ---- 全交开关：每一项都要走开 / 关两条路 ----

/// 打开时全交、关掉时降级成 +10 点。
fn assert_toggle(mut case: Win, kind: AllInKind, switch: fn(&mut ImpactRules, bool)) {
    switch(&mut case.rules, true);
    let on = case.win();
    assert_eq!(on.all_in(), Some(kind), "{kind:?} 打开时应该全交");
    assert_eq!(on.yaku().len(), 1, "全交只报这一条番种");
    assert_eq!(on.points(), 0, "全交不算点");

    switch(&mut case.rules, false);
    let off = case.win();
    assert_eq!(off.all_in(), None, "{kind:?} 关掉之后不该全交");
    assert!(
        yaku_names(&off).contains(&kind.fallback_yaku().as_str()),
        "{kind:?} 关掉之后应该降级记 +10"
    );
    assert!(off.points() >= 12 + 10);
}

#[test]
fn all_honors_toggles_between_an_all_in_and_ten_points() {
    let mut case = Win::new("1m", "1z 1z 2z 2z 3z 3z 4z 4z 5z 5z 6z 6z 7z 7z", "7z");
    case.rules.all_in.pure_seven_pairs = false;
    assert_toggle(case, AllInKind::AllHonors, |rules, on| {
        rules.all_in.all_honors = on;
    });
}

#[test]
fn pure_flush_without_a_substituting_joker_toggles() {
    let case = Win::new("1z", "1m 2m 3m 4m 5m 6m 7m 8m 9m 1m 2m 3m 5m 5m", "5m");
    assert_toggle(case, AllInKind::PureFlushNoJoker, |rules, on| {
        rules.all_in.pure_flush_no_joker = on;
    });
}

#[test]
fn a_substituting_joker_downgrades_a_pure_flush_to_ten_points() {
    let case = Win::new("1z", "1m 2m 3m 4m 5m 6m 7m 8m 9m 1m 2m 5m 5m 1z", "5m");
    let win = case.win();

    assert!(!win.is_all_in(), "财神顶了一张万子就只是清一色");
    assert!(yaku_names(&win).contains(&"pure_flush"));
    assert_eq!(win.points(), 12 + 10);
}

#[test]
fn pure_seven_pairs_toggles() {
    let case = Win::new("5z", "1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 6s 6s", "6s");
    assert_toggle(case, AllInKind::PureSevenPairs, |rules, on| {
        rules.all_in.pure_seven_pairs = on;
    });
}

#[test]
fn single_wait_toggles() {
    let mut case = Win::new("5z", "3s 3s", "3s");
    case.melds = vec![
        MeldSummary::new(MeldKind::Pon, kind("1m")),
        MeldSummary::new(MeldKind::Pon, kind("2p")),
        MeldSummary::new(MeldKind::Pon, kind("3p")),
        MeldSummary::new(MeldKind::Pon, kind("7z")),
    ];
    assert_toggle(case, AllInKind::SingleWait, |rules, on| {
        rules.all_in.single_wait = on;
    });
}

#[test]
fn the_last_tile_toggles() {
    let mut case = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    case.last_tile = true;
    assert_toggle(case, AllInKind::LastTile, |rules, on| {
        rules.all_in.last_tile = on;
    });
}

#[test]
fn a_blessed_opening_hand_toggles() {
    let mut case = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    case.blessing = true;
    assert_toggle(case, AllInKind::Blessing, |rules, on| {
        rules.all_in.blessing = on;
    });
}

#[test]
fn four_jokers_toggles() {
    let case = Win::new("5z", "5z 5z 5z 5z 1m 2m 3m 4p 5p 6p 7s 8s 1z 1z", "1z");
    assert_toggle(case, AllInKind::FourJokers, |rules, on| {
        rules.all_in.four_jokers = on;
    });
}

#[test]
fn three_kans_toggles() {
    let mut case = Win::new("5z", "3s 3s", "3s");
    case.melds = vec![
        MeldSummary::new(MeldKind::ConcealedKan, kind("1m")),
        MeldSummary::new(MeldKind::ConcealedKan, kind("2p")),
        MeldSummary::new(MeldKind::ConcealedKan, kind("3p")),
        MeldSummary::new(MeldKind::Pon, kind("7z")),
    ];
    case.rules.all_in.single_wait = false;
    assert_toggle(case, AllInKind::ThreeKans, |rules, on| {
        rules.all_in.three_kans = on;
    });
}

#[test]
fn an_eleven_honor_discard_streak_toggles() {
    let mut case = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    case.honor_streak = 11;
    assert_toggle(case, AllInKind::ElevenHonorStreak, |rules, on| {
        rules.all_in.eleven_honor_streak = on;
    });
}

#[test]
fn indicator_melds_are_triplets_and_never_count_toward_three_kans() {
    let mut case = Win::new("5z", "3s 3s", "3s");
    case.melds = vec![
        MeldSummary::new(MeldKind::IndicatorPon, kind("1m")),
        MeldSummary::new(MeldKind::IndicatorConcealed, kind("2p")),
        MeldSummary::new(MeldKind::IndicatorPon, kind("3p")),
        MeldSummary::new(MeldKind::Pon, kind("7z")),
    ];
    case.rules.all_in.single_wait = false;

    let win = case.win();
    assert!(!win.is_all_in());
    assert!(!yaku_names(&win).contains(&Yaku::ThreeKans.as_str()));
}

#[test]
fn a_kan_replacement_tile_is_worth_ten_points() {
    let mut case = Win::new("5z", "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z", "2z");
    case.rinshan = true;

    let win = case.win();
    assert!(yaku_names(&win).contains(&Yaku::RinshanKaihou.as_str()));
    assert_eq!(win.points(), 12 + 1 + 10);
}

// ---- 单节 ----

#[test]
fn a_table_opens_at_a_hundred_points_and_zero_kan_points() {
    let table = ImpactMatch::new(ImpactRules::standard(), seat(0));

    assert_eq!(table.points(), &[100; 4]);
    assert_eq!(table.kan_points(), &[0; 4]);
    assert_eq!(table.progress().dealer(), seat(0));
    assert!(table.hand().is_none());
    assert!(!table.is_finished());

    for result in table.results() {
        assert_eq!(result.point_delta, 0);
        assert_eq!(result.kan_points, 0);
    }
}

#[test]
fn hands_run_one_at_a_time() {
    let mut table = ImpactMatch::new(ImpactRules::standard(), seat(0));
    let seed = WallSeed::from_bytes([41; 32]);

    assert_eq!(table.settle_hand(), Err(MatchError::NoHandInProgress));
    table.start_hand(&seed).expect("the first hand starts");
    assert_eq!(table.start_hand(&seed), Err(MatchError::HandInProgress));
    assert_eq!(table.settle_hand(), Err(MatchError::HandNotFinished));

    let hand = table.hand().expect("a hand is running");
    assert_eq!(hand.dealer(), seat(0));
    assert_eq!(hand.dealer_streak(), 0);
    assert_eq!(hand.player(seat(0)).concealed().len(), 14);
    for index in 1..4 {
        assert_eq!(hand.player(seat(index)).concealed().len(), 13);
    }
}

#[test]
fn the_same_seed_always_deals_the_same_hand() {
    let seed = WallSeed::from_bytes([64; 32]);
    let mut first = ImpactMatch::new(ImpactRules::standard(), seat(2));
    let mut second = ImpactMatch::new(ImpactRules::standard(), seat(2));
    first.start_hand(&seed).expect("the first hand starts");
    second.start_hand(&seed).expect("the first hand starts");

    let left = first.hand().expect("a hand is running");
    let right = second.hand().expect("a hand is running");

    assert_eq!(left.joker(), right.joker());
    assert_eq!(left.indicator(), right.indicator());
    for index in 0..4 {
        assert_eq!(
            left.player(seat(index)).concealed(),
            right.player(seat(index)).concealed()
        );
    }
}
