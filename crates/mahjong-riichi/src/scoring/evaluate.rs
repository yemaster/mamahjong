use crate::WinQuery;

use super::analysis::interpretations;
use super::fu::calculate_fu;
use super::points::{base_points, payment};
use super::result::{BonusHan, WinEvaluation};
use super::yaku::{bonus_han, regular_yaku, yakuman};

pub(super) fn evaluate(query: WinQuery<'_>) -> Option<WinEvaluation> {
    let winner_is_dealer = query.seat() == query.progress().dealer();
    let player_count = query.rules().variant.seat_count().value();
    let mut best: Option<WinEvaluation> = None;

    for interpretation in interpretations(query).iter() {
        let yakuman_values = yakuman(query, interpretation);
        let (yaku, bonuses, han, fu, yakuman_multiplier) = if yakuman_values.is_empty() {
            let regular = regular_yaku(query, interpretation);
            if regular.is_empty() {
                continue;
            }
            let bonuses = bonus_han(query, interpretation);
            let yaku_han = regular.iter().map(|value| value.value()).sum::<u8>();
            let han = yaku_han
                .checked_add(bonuses.total())
                .expect("riichi hand han count fits u8");
            let fu = calculate_fu(query, interpretation, &regular);
            (regular, bonuses, han, fu, 0)
        } else {
            let multiplier = yakuman_values.iter().map(|value| value.value()).sum::<u8>();
            (yakuman_values, BonusHan::default(), 0, 0, multiplier)
        };
        let (base, limit) = base_points(query.rules(), han, fu, yakuman_multiplier);
        let payment = payment(base, winner_is_dealer, query.source());
        let candidate = WinEvaluation::new(
            interpretation.shape,
            interpretation.wait,
            yaku,
            bonuses,
            han,
            fu,
            yakuman_multiplier,
            base,
            limit,
            payment,
        );
        if best.as_ref().is_none_or(|current| {
            comparison_key(&candidate, player_count, winner_is_dealer)
                > comparison_key(current, player_count, winner_is_dealer)
        }) {
            best = Some(candidate);
        }
    }
    best
}

fn comparison_key(
    evaluation: &WinEvaluation,
    player_count: u8,
    winner_is_dealer: bool,
) -> (u32, u8, u8, u16) {
    (
        evaluation
            .payment()
            .total_received(player_count, winner_is_dealer),
        evaluation.yakuman_multiplier(),
        evaluation.han(),
        evaluation.fu(),
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        HandJudge, HandShape, MeldKind, Payment, PlayerHand, RiichiRules, RiichiScorer,
        RiichiVariant, Seat, TableProgress, Tile, TileId, TileKind, TileSet, WaitKind, Wall,
        WallSeed, WinQuery, WinSource, Yaku, YakumanValue,
    };

    struct Fixture {
        rules: RiichiRules,
        progress: TableProgress,
        player: PlayerHand,
        wall: Wall,
        winner: Seat,
        winning_tile: Tile,
        calls_occurred: bool,
    }

    impl Fixture {
        fn ron(codes: &str, winning_code: &str) -> Self {
            let rules = RiichiRules::default();
            let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
            let winner = Seat::new(RiichiVariant::Yonma, 2).expect("winner");
            let progress = TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("progress");
            let concealed = tiles(codes);
            let winning_tile = Tile::new(
                TileId::new(200),
                winning_code.parse().expect("winning kind"),
                false,
            )
            .expect("winning tile");
            Self {
                rules,
                progress,
                player: PlayerHand::scoring_fixture(concealed),
                wall: Wall::new(
                    TileSet::standard(RiichiVariant::Yonma),
                    &WallSeed::from_bytes([51; 32]),
                ),
                winner,
                winning_tile,
                calls_occurred: false,
            }
        }

        fn query(&self) -> WinQuery<'_> {
            self.query_with_source(WinSource::Discard {
                from: Seat::new(RiichiVariant::Yonma, 1).expect("discarder"),
            })
        }

        fn query_with_source(&self, source: WinSource) -> WinQuery<'_> {
            WinQuery::new(
                &self.rules,
                self.progress,
                self.winner,
                &self.player,
                self.winning_tile,
                source,
                &self.wall,
                self.calls_occurred,
            )
        }
    }

    fn tiles(codes: &str) -> Vec<Tile> {
        tiles_from(0, codes)
    }

    fn tiles_from(first_id: u16, codes: &str) -> Vec<Tile> {
        codes
            .split_whitespace()
            .enumerate()
            .map(|(index, code)| {
                Tile::new(
                    TileId::new(first_id + u16::try_from(index).expect("tile id")),
                    code.parse::<TileKind>().expect("tile kind"),
                    false,
                )
                .expect("tile")
            })
            .collect()
    }

    #[test]
    fn selects_pinfu_sanshoku_interpretation_and_calculates_ron() {
        let fixture = Fixture::ron("1m 2m 3m 1p 2p 3p 1s 2s 3s 4s 5s 7p 7p", "6s");

        let result = RiichiScorer.evaluate(fixture.query()).expect("valid win");

        assert_eq!(result.shape(), HandShape::Standard);
        assert_eq!(result.wait(), WaitKind::TwoSided);
        assert_eq!(result.fu(), 30);
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::Pinfu)
        );
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::SanshokuDoujun)
        );
        assert!(matches!(result.payment(), Payment::Ron { .. }));
    }

    #[test]
    fn seven_pairs_has_fixed_twenty_five_fu() {
        let fixture = Fixture::ron("1m 1m 2m 2m 3p 3p 4p 4p 5s 5s 6s 6s 7z", "7z");

        let result = RiichiScorer.evaluate(fixture.query()).expect("seven pairs");

        assert_eq!(result.shape(), HandShape::SevenPairs);
        assert_eq!(result.fu(), 25);
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::SevenPairs)
        );
    }

    #[test]
    fn own_discard_on_any_current_wait_prevents_ron() {
        let mut fixture = Fixture::ron("1m 2m 3m 1p 2p 3p 1s 2s 3s 4s 5s 7p 7p", "6s");
        let discarded_wait =
            Tile::new(TileId::new(201), "3s".parse().expect("tile kind"), false).expect("tile");
        fixture.player.add_scoring_fixture_discard(discarded_wait);

        assert!(RiichiScorer.evaluate(fixture.query()).is_some());
        assert!(!RiichiScorer.can_win(fixture.query()));
    }

    #[test]
    fn only_thirteen_orphans_can_rob_a_concealed_kan() {
        let from = Seat::new(RiichiVariant::Yonma, 1).expect("kan declarer");
        let ordinary = Fixture::ron("1m 2m 3m 1p 2p 3p 1s 2s 3s 4s 5s 7p 7p", "6s");
        let ordinary_query = ordinary.query_with_source(WinSource::ConcealedKan { from });
        assert!(RiichiScorer.evaluate(ordinary_query).is_some());
        assert!(!RiichiScorer.can_win(ordinary_query));

        let orphans = Fixture::ron("1m 9m 1p 9p 1s 9s 1z 2z 3z 4z 5z 6z 7z", "1m");
        let orphans_query = orphans.query_with_source(WinSource::ConcealedKan { from });
        assert!(RiichiScorer.can_win(orphans_query));

        let mut disabled = Fixture::ron("1m 9m 1p 9p 1s 9s 1z 2z 3z 4z 5z 6z 7z", "1m");
        disabled.rules.scoring.kokushi_ankan_chankan = false;
        let disabled_query = disabled.query_with_source(WinSource::ConcealedKan { from });
        assert!(!RiichiScorer.can_win(disabled_query));
    }

    #[test]
    fn thirteen_sided_orphans_uses_configured_double_yakuman() {
        let mut fixture = Fixture::ron("1m 9m 1p 9p 1s 9s 1z 2z 3z 4z 5z 6z 7z", "1m");

        let result = RiichiScorer.evaluate(fixture.query()).expect("orphans");

        assert_eq!(result.shape(), HandShape::ThirteenOrphans);
        assert_eq!(result.wait(), WaitKind::ThirteenSided);
        assert_eq!(result.yakuman_multiplier(), 2);
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::ThirteenWaitOrphans)
        );

        fixture.rules.scoring.yakuman_value = YakumanValue::StackedOnly;
        let single = RiichiScorer
            .evaluate(fixture.query())
            .expect("configured single yakuman");
        assert_eq!(single.yakuman_multiplier(), 1);
    }

    #[test]
    fn bonus_only_complete_shape_cannot_win() {
        let fixture = Fixture::ron("1m 2m 3m 4m 5m 6m 7p 8p 9p 2s 2s 5p 5p", "2s");

        assert!(RiichiScorer.evaluate(fixture.query()).is_none());
    }

    #[test]
    fn pure_yakuman_can_stack_without_special_double_rule() {
        let mut fixture = Fixture::ron("1z 1z 1z 2z 2z 2z 3z 3z 3z 4z 4z 4z 5z", "5z");
        fixture.rules.scoring.yakuman_value = YakumanValue::StackedOnly;

        let result = RiichiScorer
            .evaluate(fixture.query())
            .expect("stacked yakuman");

        assert_eq!(result.yakuman_multiplier(), 3);
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::Daisuushi)
        );
        assert!(
            result
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::Tsuuiisou)
        );
    }

    #[test]
    fn big_four_winds_uses_configured_double_yakuman() {
        let mut fixture = Fixture::ron("1z 1z 1z 2z 2z 2z 3z 3z 3z 4z 4z 4z 5z", "5z");

        let result = RiichiScorer
            .evaluate(fixture.query())
            .expect("big four winds");
        let big_four_winds = result
            .yaku()
            .iter()
            .find(|value| value.yaku() == Yaku::Daisuushi)
            .expect("big four winds yaku");
        assert_eq!(big_four_winds.value(), 2);

        fixture.rules.scoring.yakuman_value = YakumanValue::StackedOnly;
        let configured_single = RiichiScorer
            .evaluate(fixture.query())
            .expect("configured single big four winds");
        let big_four_winds = configured_single
            .yaku()
            .iter()
            .find(|value| value.yaku() == Yaku::Daisuushi)
            .expect("big four winds yaku");
        assert_eq!(big_four_winds.value(), 1);
    }

    #[test]
    fn old_wheel_yaku_is_isolated_behind_rule_toggle() {
        let mut fixture = Fixture::ron("2p 2p 3p 3p 4p 4p 5p 5p 6p 6p 7p 7p 8p", "8p");
        fixture.calls_occurred = true;
        let normal = RiichiScorer
            .evaluate(fixture.query())
            .expect("ordinary seven pairs");
        assert_eq!(normal.yakuman_multiplier(), 0);

        fixture.rules.scoring.old_yaku = true;
        let old_yaku = RiichiScorer.evaluate(fixture.query()).expect("old yaku");
        assert_eq!(old_yaku.yakuman_multiplier(), 1);
        assert!(
            old_yaku
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::Daisharin)
        );
    }

    #[test]
    fn kuitan_decides_whether_an_open_all_simples_hand_has_a_yaku() {
        let mut fixture = Fixture::ron("5m 6m 7m 2p 3p 4p 5s 6s 8p 8p", "7s");
        fixture.calls_occurred = true;
        fixture.player.add_scoring_fixture_meld(
            MeldKind::Chi,
            tiles_from(100, "2m 3m 4m"),
            Some(Seat::new(RiichiVariant::Yonma, 1).expect("discarder")),
        );

        let open = RiichiScorer.evaluate(fixture.query()).expect("kuitan win");
        assert_eq!(
            open.yaku()
                .iter()
                .map(|value| value.yaku())
                .collect::<Vec<_>>(),
            [Yaku::Tanyao]
        );

        fixture.rules.calls.kuitan = false;
        assert!(RiichiScorer.evaluate(fixture.query()).is_none());
        assert!(!RiichiScorer.can_win(fixture.query()));
    }

    #[test]
    fn kuitan_never_affects_a_closed_all_simples_hand() {
        let mut fixture = Fixture::ron("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 6s 8p 8p", "7s");
        fixture.rules.calls.kuitan = false;

        let closed = RiichiScorer.evaluate(fixture.query()).expect("closed win");
        assert!(
            closed
                .yaku()
                .iter()
                .any(|value| value.yaku() == Yaku::Tanyao)
        );
    }
}

/// 符数规则回归：底 20 符，门清荣和 +10，自摸 +2，平和自摸固定 20，
/// 七对子固定 25，副露且只有 20 符时抬到 30，役牌雀头 +2，
/// 坎张/边张/单骑 +2，刻子按「明/暗 × 中张/幺九 × 是否杠」查表。
#[cfg(test)]
mod fu_rules {
    use crate::{
        DrawSource, MeldKind, PlayerHand, RiichiRules, RiichiScorer, RiichiVariant, Seat,
        TableProgress, Tile, TileId, TileKind, TileSet, Wall, WallSeed, WinQuery, WinSource,
    };

    fn tiles_from(first_id: u16, codes: &str) -> Vec<Tile> {
        codes
            .split_whitespace()
            .enumerate()
            .map(|(index, code)| {
                Tile::new(
                    TileId::new(first_id + u16::try_from(index).expect("tile id")),
                    code.parse::<TileKind>().expect("tile kind"),
                    false,
                )
                .expect("tile")
            })
            .collect()
    }

    /// 算一手牌的符数。`melds` 里给出副露，空的就是门清。
    fn fu_of(codes: &str, winning: &str, tsumo: bool, melds: &[(MeldKind, &str)]) -> u16 {
        let rules = RiichiRules::default();
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("dealer");
        let winner = Seat::new(RiichiVariant::Yonma, 2).expect("winner");
        let progress = TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("progress");
        let mut player = PlayerHand::scoring_fixture(tiles_from(0, codes));
        /* 牌河空着的话自摸会被判成地和，随便垫一张不相干的牌。 */
        player.add_scoring_fixture_discard(
            Tile::new(TileId::new(250), "4z".parse().expect("kind"), false).expect("tile"),
        );
        for (index, (kind, meld_codes)) in melds.iter().enumerate() {
            player.add_scoring_fixture_meld(
                *kind,
                tiles_from(
                    100 + u16::try_from(index).expect("meld id") * 10,
                    meld_codes,
                ),
                (!matches!(kind, MeldKind::ConcealedKan))
                    .then(|| Seat::new(RiichiVariant::Yonma, 1).expect("discarder")),
            );
        }
        let winning_tile = Tile::new(
            TileId::new(200),
            winning.parse().expect("winning kind"),
            false,
        )
        .expect("winning tile");
        let wall = Wall::new(
            TileSet::standard(RiichiVariant::Yonma),
            &WallSeed::from_bytes([51; 32]),
        );
        let source = if tsumo {
            WinSource::Tsumo(DrawSource::LiveWall)
        } else {
            WinSource::Discard {
                from: Seat::new(RiichiVariant::Yonma, 1).expect("discarder"),
            }
        };
        RiichiScorer
            .evaluate(WinQuery::new(
                &rules,
                progress,
                winner,
                &player,
                winning_tile,
                source,
                &wall,
                !melds.is_empty(),
            ))
            .expect("win")
            .fu()
    }

    #[test]
    fn closed_ron_adds_ten_and_rounds_up() {
        /* 20 底 + 10 门清荣和 + 4 中张暗刻 = 34 → 40 */
        assert_eq!(
            fu_of("5p 5p 5p 2m 3m 4m 5m 6m 7m 2s 3s 8p 8p", "4s", false, &[]),
            40
        );
        /* 20 + 10 + 8 幺九暗刻 = 38 → 40 */
        assert_eq!(
            fu_of("5z 5z 5z 2m 3m 4m 5p 6p 7p 2s 3s 8p 8p", "4s", false, &[]),
            40
        );
    }

    #[test]
    fn wait_shape_adds_two_only_for_kanchan_penchan_tanki() {
        /* 坎张：20 + 10 + 2 = 32 → 40 */
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 4p 5s 6s 7s 8p 8p", "3p", false, &[]),
            40
        );
        /* 单骑：20 + 10 + 2 = 32 → 40 */
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 6s 7s 8p", "8p", false, &[]),
            40
        );
        /* 两面平和荣和：20 + 10 = 30，不再加 */
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 6s 8p 8p", "7s", false, &[]),
            30
        );
    }

    #[test]
    fn pinfu_tsumo_is_flat_twenty() {
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 6s 7s 8p 8p", "7s", true, &[]),
            20
        );
    }

    #[test]
    fn tsumo_adds_two_and_keeps_triplets_concealed() {
        /* 双碰自摸：20 + 2 自摸 + 8 幺九暗刻 = 30 */
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 5s 5z 5z 5z", "5z", true, &[]),
            30
        );
        /* 同型荣和：那副刻子算明刻，20 + 10 + 4 = 34 → 40 */
        assert_eq!(
            fu_of("2m 3m 4m 5m 6m 7m 2p 3p 4p 5s 5s 5z 5z", "5z", false, &[]),
            40
        );
    }

    #[test]
    fn value_pair_adds_two() {
        /* 20 + 2 自摸 + 8 幺九暗刻 + 2 白雀头 = 32 → 40 */
        assert_eq!(
            fu_of("1m 1m 1m 2m 3m 4m 5p 6p 7p 2s 3s 4s 5z 5z", "4s", true, &[]),
            40
        );
    }

    #[test]
    fn kan_uses_the_quadrupled_row_of_the_table() {
        /* 暗杠幺九 32 符：20 + 2 自摸 + 32 + 2 白雀头 = 56 → 60 */
        assert_eq!(
            fu_of(
                "2m 3m 4m 5m 6m 7m 2p 3p 4p 5z 5z",
                "4m",
                true,
                &[(MeldKind::ConcealedKan, "1z 1z 1z 1z")]
            ),
            60
        );
        /* 明杠幺九 16 符：副露没有门清荣和，20 + 16 = 36 → 40 */
        assert_eq!(
            fu_of(
                "2m 3m 4m 5m 6m 7m 2p 3p 5z 5z",
                "4p",
                false,
                &[(MeldKind::OpenKan, "1z 1z 1z 1z")]
            ),
            40
        );
    }

    #[test]
    fn open_hand_worth_only_twenty_is_lifted_to_thirty() {
        /* 食い平和：副露、全顺子、两面、非役牌雀头，正好 20 符要抬到 30。 */
        assert_eq!(
            fu_of(
                "5m 6m 7m 2p 3p 4p 5s 6s 8p 8p",
                "7s",
                false,
                &[(MeldKind::Chi, "2m 3m 4m")]
            ),
            30
        );
    }

    #[test]
    fn seven_pairs_ignores_every_other_term() {
        assert_eq!(
            fu_of("1m 1m 2m 2m 3p 3p 4p 4p 5s 5s 6s 6s 7z", "7z", false, &[]),
            25
        );
    }
}
