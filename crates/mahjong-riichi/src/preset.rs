use std::num::NonZeroU32;

use mahjong_core::{PresetId, PresetRef};

use crate::{
    AbortiveDrawRules, BonusRules, DealerContinuation, MatchLength, MatchRules, PlacementUma,
    RedFives, RiichiRules, RiichiVariant, RonResolution, ScoringRules, SettlementRules,
    YakumanValue,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiichiPreset {
    JpmlA,
    Saikouisen,
    MLeague,
}

impl RiichiPreset {
    pub const ALL: [Self; 3] = [Self::JpmlA, Self::Saikouisen, Self::MLeague];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::JpmlA => "jpml-a",
            Self::Saikouisen => "saikouisen",
            Self::MLeague => "m-league",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::JpmlA => "日本职业麻将联盟 A 规则",
            Self::Saikouisen => "最高位战规则",
            Self::MLeague => "M League 规则",
        }
    }

    #[must_use]
    pub const fn revision(self) -> NonZeroU32 {
        NonZeroU32::MIN
    }

    #[must_use]
    pub fn preset_ref(self) -> PresetRef {
        PresetRef::new(
            PresetId::parse(self.id()).expect("built-in preset IDs are valid"),
            self.revision(),
        )
    }

    #[must_use]
    pub fn find(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }

    #[must_use]
    pub fn rules(self) -> RiichiRules {
        let mut rules = competition_base();

        match self {
            Self::JpmlA => {
                rules.scoring.kiriage_mangan = false;
                rules.bonuses.ippatsu = false;
                rules.bonuses.ura_dora = false;
                rules.bonuses.kan_dora = false;
                rules.settlement.uma = PlacementUma::JpmlA;
            }
            Self::Saikouisen => {
                rules.bonuses.ippatsu = true;
                rules.bonuses.ura_dora = true;
                rules.bonuses.kan_dora = true;
                rules.settlement.uma = PlacementUma::Fixed {
                    values: vec![30, 10, -10, -30],
                };
            }
            Self::MLeague => {
                rules.match_rules.initial_points = 25_000;
                rules.match_rules.return_points = 30_000;
                rules.bonuses.red_fives =
                    RedFives::new(1, 1, 1).expect("built-in red fives are valid");
                rules.bonuses.ippatsu = true;
                rules.bonuses.ura_dora = true;
                rules.bonuses.kan_dora = true;
                rules.settlement.uma = PlacementUma::Fixed {
                    values: vec![30, 10, -10, -30],
                };
            }
        }

        rules
            .validate()
            .expect("built-in preset configurations are valid");
        rules
    }
}

fn competition_base() -> RiichiRules {
    RiichiRules {
        variant: RiichiVariant::Yonma,
        match_rules: MatchRules {
            length: MatchLength::Hanchan,
            initial_points: 30_000,
            return_points: 30_000,
            tobi: false,
            dealer_continuation: DealerContinuation::WinOrTenpai,
            agari_yame: false,
        },
        scoring: ScoringRules {
            kiriage_mangan: true,
            old_yaku: false,
            yakuman_value: YakumanValue::StackedOnly,
            nagashi_mangan: false,
            kazoe_yakuman: false,
            kokushi_ankan_chankan: false,
        },
        bonuses: BonusRules {
            red_fives: RedFives::new(0, 0, 0).expect("zero red fives are valid"),
            ippatsu: false,
            ura_dora: false,
            kan_dora: false,
        },
        abortive_draws: AbortiveDrawRules {
            four_winds: false,
            four_kans: false,
            nine_terminals: false,
            four_riichi: false,
        },
        settlement: SettlementRules {
            uma: PlacementUma::Fixed {
                values: vec![30, 10, -10, -30],
            },
            noten_payment: 3_000,
            ron_resolution: RonResolution::HeadBump,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{PlacementUma, RiichiPreset, Suit};

    #[test]
    fn catalog_ids_and_revisions_are_stable() {
        let metadata: Vec<_> = RiichiPreset::ALL
            .into_iter()
            .map(|preset| (preset.id(), preset.revision().get()))
            .collect();

        assert_eq!(
            metadata,
            vec![("jpml-a", 1), ("saikouisen", 1), ("m-league", 1)]
        );
        assert_eq!(RiichiPreset::find("m-league"), Some(RiichiPreset::MLeague));
        assert_eq!(RiichiPreset::find("unknown"), None);
    }

    #[test]
    fn jpml_a_revision_one_is_locked() {
        let rules = RiichiPreset::JpmlA.rules();

        assert_eq!(rules.match_rules.initial_points, 30_000);
        assert_eq!(rules.match_rules.return_points, 30_000);
        assert!(!rules.match_rules.tobi);
        assert!(!rules.scoring.kiriage_mangan);
        assert!(!rules.scoring.kazoe_yakuman);
        assert_eq!(rules.bonuses.red_fives.total(), 0);
        assert!(!rules.bonuses.ippatsu);
        assert!(!rules.bonuses.ura_dora);
        assert!(!rules.bonuses.kan_dora);
        assert_eq!(rules.settlement.uma, PlacementUma::JpmlA);
    }

    #[test]
    fn saikouisen_revision_one_is_locked() {
        let rules = RiichiPreset::Saikouisen.rules();

        assert_eq!(rules.match_rules.initial_points, 30_000);
        assert_eq!(rules.match_rules.return_points, 30_000);
        assert_eq!(rules.bonuses.red_fives.total(), 0);
        assert!(rules.bonuses.ippatsu);
        assert!(rules.bonuses.ura_dora);
        assert!(rules.bonuses.kan_dora);
        assert_eq!(
            rules.settlement.uma,
            PlacementUma::Fixed {
                values: vec![30, 10, -10, -30]
            }
        );
    }

    #[test]
    fn m_league_revision_one_is_locked() {
        let rules = RiichiPreset::MLeague.rules();

        assert_eq!(rules.match_rules.initial_points, 25_000);
        assert_eq!(rules.match_rules.return_points, 30_000);
        assert_eq!(rules.bonuses.red_fives.for_suit(Suit::Man), 1);
        assert_eq!(rules.bonuses.red_fives.for_suit(Suit::Pin), 1);
        assert_eq!(rules.bonuses.red_fives.for_suit(Suit::Sou), 1);
        assert!(rules.bonuses.ippatsu);
        assert!(rules.bonuses.ura_dora);
        assert!(rules.bonuses.kan_dora);
    }

    #[test]
    fn official_presets_share_the_competition_invariants() {
        for preset in RiichiPreset::ALL {
            let value = serde_json::to_value(preset.rules()).expect("serialize preset");

            assert_eq!(value["variant"], json!("yonma"));
            assert_eq!(value["match_rules"]["length"], json!("hanchan"));
            assert_eq!(value["match_rules"]["agari_yame"], json!(false));
            assert_eq!(value["scoring"]["old_yaku"], json!(false));
            assert_eq!(value["scoring"]["yakuman_value"], json!("stacked_only"));
            assert_eq!(value["scoring"]["nagashi_mangan"], json!(false));
            assert_eq!(value["scoring"]["kazoe_yakuman"], json!(false));
            assert_eq!(value["scoring"]["kokushi_ankan_chankan"], json!(false));
            assert_eq!(
                value["abortive_draws"],
                json!({
                    "four_winds": false,
                    "four_kans": false,
                    "nine_terminals": false,
                    "four_riichi": false
                })
            );
            assert_eq!(value["settlement"]["ron_resolution"], json!("head_bump"));
        }
    }
}
