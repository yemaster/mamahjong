use serde::{Deserialize, Serialize};

use crate::{RedFives, RiichiVariant};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchLength {
    EastOnly,
    Hanchan,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DealerContinuation {
    WinOnly,
    WinOrTenpai,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum YakumanValue {
    StackedOnly,
    DoubleVariantsAndStacked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RonResolution {
    HeadBump,
    Multiple,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum PlacementUma {
    Fixed { values: Vec<i16> },
    JpmlA,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRules {
    pub length: MatchLength,
    pub initial_points: u32,
    pub return_points: u32,
    pub tobi: bool,
    pub dealer_continuation: DealerContinuation,
    pub agari_yame: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringRules {
    pub kiriage_mangan: bool,
    pub old_yaku: bool,
    pub yakuman_value: YakumanValue,
    pub nagashi_mangan: bool,
    pub kazoe_yakuman: bool,
    pub kokushi_ankan_chankan: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BonusRules {
    pub red_fives: RedFives,
    pub ippatsu: bool,
    pub ura_dora: bool,
    pub kan_dora: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbortiveDrawRules {
    pub four_winds: bool,
    pub four_kans: bool,
    pub nine_terminals: bool,
    pub four_riichi: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementRules {
    pub uma: PlacementUma,
    pub noten_payment: u32,
    pub ron_resolution: RonResolution,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiichiRules {
    pub variant: RiichiVariant,
    pub match_rules: MatchRules,
    pub scoring: ScoringRules,
    pub bonuses: BonusRules,
    pub abortive_draws: AbortiveDrawRules,
    pub settlement: SettlementRules,
}

impl RiichiRules {
    #[must_use]
    pub fn standard(variant: RiichiVariant) -> Self {
        let (abortive_draws, uma) = match variant {
            RiichiVariant::Yonma => (
                AbortiveDrawRules {
                    four_winds: true,
                    four_kans: true,
                    nine_terminals: true,
                    four_riichi: true,
                },
                PlacementUma::Fixed {
                    values: vec![30, 10, -10, -30],
                },
            ),
            RiichiVariant::Sanma => (
                AbortiveDrawRules {
                    four_winds: false,
                    four_kans: true,
                    nine_terminals: true,
                    four_riichi: false,
                },
                PlacementUma::Fixed {
                    values: vec![30, 0, -30],
                },
            ),
        };

        Self {
            variant,
            match_rules: MatchRules {
                length: MatchLength::Hanchan,
                initial_points: 25_000,
                return_points: 30_000,
                tobi: true,
                dealer_continuation: DealerContinuation::WinOrTenpai,
                agari_yame: true,
            },
            scoring: ScoringRules {
                kiriage_mangan: true,
                old_yaku: false,
                yakuman_value: YakumanValue::DoubleVariantsAndStacked,
                nagashi_mangan: true,
                kazoe_yakuman: true,
                kokushi_ankan_chankan: true,
            },
            bonuses: BonusRules {
                red_fives: variant.default_red_fives(),
                ippatsu: true,
                ura_dora: true,
                kan_dora: true,
            },
            abortive_draws,
            settlement: SettlementRules {
                uma,
                noten_payment: 3_000,
                ron_resolution: RonResolution::Multiple,
            },
        }
    }
}

impl Default for RiichiRules {
    fn default() -> Self {
        Self::standard(RiichiVariant::Yonma)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DealerContinuation, PlacementUma, RiichiRules, RiichiVariant, Suit};

    #[test]
    fn yonma_default_expands_every_rule() {
        let rules = RiichiRules::default();

        assert_eq!(rules.variant, RiichiVariant::Yonma);
        assert_eq!(rules.match_rules.initial_points, 25_000);
        assert_eq!(rules.match_rules.return_points, 30_000);
        assert!(rules.match_rules.tobi);
        assert_eq!(
            rules.match_rules.dealer_continuation,
            DealerContinuation::WinOrTenpai
        );
        assert_eq!(rules.bonuses.red_fives.total(), 3);
        assert!(rules.abortive_draws.four_winds);
        assert!(rules.abortive_draws.four_riichi);
        assert_eq!(
            rules.settlement.uma,
            PlacementUma::Fixed {
                values: vec![30, 10, -10, -30]
            }
        );
    }

    #[test]
    fn sanma_default_removes_impossible_options() {
        let rules = RiichiRules::standard(RiichiVariant::Sanma);

        assert_eq!(rules.bonuses.red_fives.for_suit(Suit::Man), 0);
        assert_eq!(rules.bonuses.red_fives.total(), 2);
        assert!(!rules.abortive_draws.four_winds);
        assert!(!rules.abortive_draws.four_riichi);
        assert_eq!(
            rules.settlement.uma,
            PlacementUma::Fixed {
                values: vec![30, 0, -30]
            }
        );
    }
}
