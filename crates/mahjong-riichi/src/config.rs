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

/// How the north tiles behave in three-player riichi mahjong.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SanmaNorthRule {
    /// A north tile may be extracted for one bonus han and a rinshan draw.
    #[default]
    NukiDora,
    /// North is an unconditional value tile and cannot be extracted.
    Yakuhai,
}

impl SanmaNorthRule {
    fn is_nuki_dora(value: &Self) -> bool {
        matches!(value, Self::NukiDora)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum YakumanValue {
    StackedOnly,
    DoubleVariantsAndStacked,
}

/// Restriction on discarding right after a chi or pon.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KuikaeRule {
    /// The called tile kind and, for chi, the suji tile are both forbidden.
    Forbidden,
    /// Only the called tile kind is forbidden.
    SameTileOnly,
    /// No restriction.
    Allowed,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThinkingTimeRules {
    pub base_seconds: u16,
    pub reserve_seconds: u16,
}

impl ThinkingTimeRules {
    #[must_use]
    pub const fn base_ms(self) -> u64 {
        self.base_seconds as u64 * 1_000
    }

    #[must_use]
    pub const fn reserve_ms(self) -> u32 {
        self.reserve_seconds as u32 * 1_000
    }
}

impl Default for ThinkingTimeRules {
    fn default() -> Self {
        Self {
            base_seconds: 5,
            reserve_seconds: 20,
        }
    }
}

const fn default_first_place_required_points() -> u32 {
    30_000
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRules {
    pub length: MatchLength,
    pub initial_points: u32,
    pub return_points: u32,
    #[serde(default = "default_first_place_required_points")]
    pub first_place_required_points: u32,
    #[serde(default)]
    pub thinking_time: ThinkingTimeRules,
    pub tobi: bool,
    pub dealer_continuation: DealerContinuation,
    pub agari_yame: bool,
    #[serde(default, skip_serializing_if = "SanmaNorthRule::is_nuki_dora")]
    pub north: SanmaNorthRule,
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

/// Rules that only take effect once a hand is opened by a call.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallRules {
    pub kuitan: bool,
    pub kuikae: KuikaeRule,
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
    pub calls: CallRules,
    pub bonuses: BonusRules,
    pub abortive_draws: AbortiveDrawRules,
    pub settlement: SettlementRules,
}

impl RiichiRules {
    #[must_use]
    pub fn standard(variant: RiichiVariant) -> Self {
        let (abortive_draws, uma, noten_payment) = match variant {
            RiichiVariant::Yonma => (
                AbortiveDrawRules {
                    four_winds: false,
                    four_kans: false,
                    nine_terminals: false,
                    four_riichi: false,
                },
                PlacementUma::Fixed {
                    values: vec![30, 10, -10, -30],
                },
                3_000,
            ),
            RiichiVariant::Sanma => (
                AbortiveDrawRules {
                    four_winds: false,
                    four_kans: false,
                    nine_terminals: false,
                    four_riichi: false,
                },
                PlacementUma::Fixed {
                    values: vec![30, 0, -30],
                },
                2_000,
            ),
        };

        Self {
            variant,
            match_rules: MatchRules {
                length: MatchLength::Hanchan,
                initial_points: 25_000,
                return_points: 25_000,
                first_place_required_points: 30_000,
                thinking_time: ThinkingTimeRules::default(),
                tobi: true,
                dealer_continuation: DealerContinuation::WinOrTenpai,
                agari_yame: true,
                north: SanmaNorthRule::NukiDora,
            },
            scoring: ScoringRules {
                kiriage_mangan: false,
                old_yaku: false,
                yakuman_value: YakumanValue::DoubleVariantsAndStacked,
                nagashi_mangan: true,
                kazoe_yakuman: true,
                kokushi_ankan_chankan: true,
            },
            calls: CallRules {
                kuitan: true,
                kuikae: KuikaeRule::Forbidden,
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
                noten_payment,
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
    use crate::{DealerContinuation, KuikaeRule, PlacementUma, RiichiRules, RiichiVariant, Suit};

    #[test]
    fn yonma_default_expands_every_rule() {
        let rules = RiichiRules::default();

        assert_eq!(rules.variant, RiichiVariant::Yonma);
        assert_eq!(rules.match_rules.initial_points, 25_000);
        assert_eq!(rules.match_rules.return_points, 25_000);
        assert_eq!(rules.match_rules.first_place_required_points, 30_000);
        assert_eq!(rules.match_rules.thinking_time.base_seconds, 5);
        assert_eq!(rules.match_rules.thinking_time.reserve_seconds, 20);
        assert!(rules.match_rules.tobi);
        assert!(rules.scoring.kokushi_ankan_chankan);
        assert_eq!(rules.match_rules.north, crate::SanmaNorthRule::NukiDora);
        assert_eq!(rules.settlement.noten_payment, 3_000);
        assert_eq!(
            rules.match_rules.dealer_continuation,
            DealerContinuation::WinOrTenpai
        );
        assert_eq!(rules.bonuses.red_fives.total(), 3);
        assert!(!rules.abortive_draws.four_winds);
        assert!(!rules.abortive_draws.four_riichi);
        assert!(rules.calls.kuitan);
        assert_eq!(rules.calls.kuikae, KuikaeRule::Forbidden);
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
        assert_eq!(rules.match_rules.north, crate::SanmaNorthRule::NukiDora);
        assert_eq!(rules.settlement.noten_payment, 2_000);
        assert!(rules.scoring.kokushi_ankan_chankan);
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
