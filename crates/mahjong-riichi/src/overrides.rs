use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{PresetId, PresetRef};
use serde::{Deserialize, Serialize};

use crate::{
    DealerContinuation, KuikaeRule, MatchLength, PlacementUma, RedFives, RiichiPreset, RiichiRules,
    RiichiVariant, RonResolution, SanmaNorthRule, ScoringRules, ThinkingTimeRules,
    ValidationErrors, YakumanValue,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresetRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RoomRuleRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetRequest>,
    #[serde(default)]
    pub overrides: RiichiRuleOverrides,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiichiRuleOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_rules: Option<MatchRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ScoringRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<CallRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonuses: Option<BonusRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abortive_draws: Option<AbortiveDrawRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<SettlementRuleOverrides>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRuleOverrides {
    pub length: Option<MatchLength>,
    pub initial_points: Option<u32>,
    pub return_points: Option<u32>,
    pub first_place_required_points: Option<u32>,
    pub thinking_time: Option<ThinkingTimeRules>,
    pub tobi: Option<bool>,
    pub dealer_continuation: Option<DealerContinuation>,
    pub agari_yame: Option<bool>,
    pub north: Option<SanmaNorthRule>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringRuleOverrides {
    pub kiriage_mangan: Option<bool>,
    pub old_yaku: Option<bool>,
    pub yakuman_value: Option<YakumanValue>,
    pub nagashi_mangan: Option<bool>,
    pub kazoe_yakuman: Option<bool>,
    pub kokushi_ankan_chankan: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallRuleOverrides {
    pub kuitan: Option<bool>,
    pub kuikae: Option<KuikaeRule>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BonusRuleOverrides {
    pub red_fives: Option<RedFiveOverrides>,
    pub ippatsu: Option<bool>,
    pub ura_dora: Option<bool>,
    pub kan_dora: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedFiveOverrides {
    pub man: Option<u8>,
    pub pin: Option<u8>,
    pub sou: Option<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbortiveDrawRuleOverrides {
    pub four_winds: Option<bool>,
    pub four_kans: Option<bool>,
    pub nine_terminals: Option<bool>,
    pub four_riichi: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementRuleOverrides {
    pub uma: Option<PlacementUma>,
    pub noten_payment: Option<u32>,
    pub ron_resolution: Option<RonResolution>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRiichiRules {
    rules: RiichiRules,
    preset: Option<PresetRef>,
}

impl ResolvedRiichiRules {
    #[must_use]
    pub const fn rules(&self) -> &RiichiRules {
        &self.rules
    }

    #[must_use]
    pub const fn preset(&self) -> Option<&PresetRef> {
        self.preset.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (RiichiRules, Option<PresetRef>) {
        (self.rules, self.preset)
    }
}

impl RoomRuleRequest {
    pub fn resolve(
        self,
        variant: RiichiVariant,
    ) -> Result<ResolvedRiichiRules, RuleResolutionError> {
        let (mut rules, preset) = match self.preset {
            Some(request) => resolve_preset(request, variant)?,
            None => (RiichiRules::standard(variant), None),
        };

        self.overrides.apply_to(&mut rules);
        rules.validate().map_err(RuleResolutionError::Validation)?;

        Ok(ResolvedRiichiRules { rules, preset })
    }
}

impl RiichiRuleOverrides {
    fn apply_to(self, rules: &mut RiichiRules) {
        if let Some(overrides) = self.match_rules {
            apply_if_some(&mut rules.match_rules.length, overrides.length);
            apply_if_some(
                &mut rules.match_rules.initial_points,
                overrides.initial_points,
            );
            apply_if_some(
                &mut rules.match_rules.return_points,
                overrides.return_points,
            );
            apply_if_some(
                &mut rules.match_rules.first_place_required_points,
                overrides.first_place_required_points,
            );
            apply_if_some(
                &mut rules.match_rules.thinking_time,
                overrides.thinking_time,
            );
            apply_if_some(&mut rules.match_rules.tobi, overrides.tobi);
            apply_if_some(
                &mut rules.match_rules.dealer_continuation,
                overrides.dealer_continuation,
            );
            apply_if_some(&mut rules.match_rules.agari_yame, overrides.agari_yame);
            apply_if_some(&mut rules.match_rules.north, overrides.north);
        }

        if let Some(overrides) = self.scoring {
            apply_scoring_overrides(&mut rules.scoring, overrides);
        }

        if let Some(overrides) = self.calls {
            apply_if_some(&mut rules.calls.kuitan, overrides.kuitan);
            apply_if_some(&mut rules.calls.kuikae, overrides.kuikae);
        }

        if let Some(overrides) = self.bonuses {
            if let Some(red_fives) = overrides.red_fives {
                let current = rules.bonuses.red_fives;
                rules.bonuses.red_fives = RedFives::new_unchecked(
                    red_fives
                        .man
                        .unwrap_or_else(|| current.for_suit(crate::Suit::Man)),
                    red_fives
                        .pin
                        .unwrap_or_else(|| current.for_suit(crate::Suit::Pin)),
                    red_fives
                        .sou
                        .unwrap_or_else(|| current.for_suit(crate::Suit::Sou)),
                );
            }
            apply_if_some(&mut rules.bonuses.ippatsu, overrides.ippatsu);
            apply_if_some(&mut rules.bonuses.ura_dora, overrides.ura_dora);
            apply_if_some(&mut rules.bonuses.kan_dora, overrides.kan_dora);
        }

        if let Some(overrides) = self.abortive_draws {
            apply_if_some(&mut rules.abortive_draws.four_winds, overrides.four_winds);
            apply_if_some(&mut rules.abortive_draws.four_kans, overrides.four_kans);
            apply_if_some(
                &mut rules.abortive_draws.nine_terminals,
                overrides.nine_terminals,
            );
            apply_if_some(&mut rules.abortive_draws.four_riichi, overrides.four_riichi);
        }

        if let Some(overrides) = self.settlement {
            apply_if_some(&mut rules.settlement.uma, overrides.uma);
            apply_if_some(&mut rules.settlement.noten_payment, overrides.noten_payment);
            apply_if_some(
                &mut rules.settlement.ron_resolution,
                overrides.ron_resolution,
            );
        }
    }
}

fn apply_scoring_overrides(scoring: &mut ScoringRules, overrides: ScoringRuleOverrides) {
    apply_if_some(&mut scoring.kiriage_mangan, overrides.kiriage_mangan);
    apply_if_some(&mut scoring.old_yaku, overrides.old_yaku);
    apply_if_some(&mut scoring.yakuman_value, overrides.yakuman_value);
    apply_if_some(&mut scoring.nagashi_mangan, overrides.nagashi_mangan);
    apply_if_some(&mut scoring.kazoe_yakuman, overrides.kazoe_yakuman);
    apply_if_some(
        &mut scoring.kokushi_ankan_chankan,
        overrides.kokushi_ankan_chankan,
    );
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

fn resolve_preset(
    request: PresetRequest,
    variant: RiichiVariant,
) -> Result<(RiichiRules, Option<PresetRef>), RuleResolutionError> {
    if PresetId::parse(request.id.as_str()).is_err() {
        return Err(RuleResolutionError::InvalidPresetId { id: request.id });
    }

    let preset =
        RiichiPreset::find(&request.id).ok_or_else(|| RuleResolutionError::UnknownPreset {
            id: request.id.clone(),
        })?;

    let requested_revision = request.revision.unwrap_or(preset.revision().get());
    if requested_revision != preset.revision().get() {
        return Err(RuleResolutionError::UnsupportedPresetRevision {
            id: request.id,
            revision: requested_revision,
        });
    }
    if !matches!(variant, RiichiVariant::Yonma) {
        return Err(RuleResolutionError::PresetVariantMismatch {
            id: request.id,
            variant,
        });
    }

    Ok((preset.rules(), Some(preset.preset_ref())))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleResolutionError {
    InvalidPresetId { id: String },
    UnknownPreset { id: String },
    UnsupportedPresetRevision { id: String, revision: u32 },
    PresetVariantMismatch { id: String, variant: RiichiVariant },
    Validation(ValidationErrors),
}

impl RuleResolutionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidPresetId { .. } => "rules.preset.invalid_id",
            Self::UnknownPreset { .. } => "rules.preset.unknown",
            Self::UnsupportedPresetRevision { .. } => "rules.preset.unsupported_revision",
            Self::PresetVariantMismatch { .. } => "rules.preset.variant_mismatch",
            Self::Validation(_) => "request.invalid_rule_config",
        }
    }
}

impl Display for RuleResolutionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPresetId { id } => write!(formatter, "invalid preset ID: {id:?}"),
            Self::UnknownPreset { id } => write!(formatter, "unknown preset: {id}"),
            Self::UnsupportedPresetRevision { id, revision } => {
                write!(formatter, "unsupported preset revision: {id}@{revision}")
            }
            Self::PresetVariantMismatch { id, variant } => {
                write!(formatter, "preset {id} does not support {variant:?}")
            }
            Self::Validation(errors) => Display::fmt(errors, formatter),
        }
    }
}

impl Error for RuleResolutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(errors) => Some(errors),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{RiichiVariant, RoomRuleRequest, RuleResolutionError, Suit};

    #[test]
    fn empty_request_resolves_to_expanded_variant_default() {
        let resolved = RoomRuleRequest::default()
            .resolve(RiichiVariant::Sanma)
            .expect("sanma defaults");

        assert_eq!(resolved.rules().variant, RiichiVariant::Sanma);
        assert_eq!(resolved.rules().bonuses.red_fives.total(), 2);
        assert!(resolved.preset().is_none());
    }

    #[test]
    fn applies_partial_overrides_on_top_of_preset() {
        let request: RoomRuleRequest = serde_json::from_value(json!({
            "preset": {"id": "m-league", "revision": 1},
            "overrides": {
                "match_rules": {"tobi": true},
                "bonuses": {"red_fives": {"pin": 0}},
                "scoring": {"old_yaku": true}
            }
        }))
        .expect("valid request");

        let resolved = request
            .resolve(RiichiVariant::Yonma)
            .expect("valid overrides");

        assert!(resolved.rules().match_rules.tobi);
        assert!(resolved.rules().scoring.old_yaku);
        assert_eq!(resolved.rules().bonuses.red_fives.for_suit(Suit::Man), 1);
        assert_eq!(resolved.rules().bonuses.red_fives.for_suit(Suit::Pin), 0);
        assert_eq!(resolved.rules().bonuses.red_fives.for_suit(Suit::Sou), 1);
        assert_eq!(resolved.preset().expect("preset").id().as_str(), "m-league");
    }

    #[test]
    fn rejects_unknown_fields_at_every_override_level() {
        let top_level = serde_json::from_value::<RoomRuleRequest>(json!({
            "overrides": {},
            "variant": "yonma"
        }));
        let nested = serde_json::from_value::<RoomRuleRequest>(json!({
            "overrides": {"scoring": {"kiriage": true}}
        }));

        assert!(top_level.is_err());
        assert!(nested.is_err());
    }

    #[test]
    fn validation_runs_after_all_overrides_are_applied() {
        let request: RoomRuleRequest = serde_json::from_value(json!({
            "overrides": {
                "bonuses": {"red_fives": {"man": 5}},
                "abortive_draws": {"four_winds": true}
            }
        }))
        .expect("structurally valid request");

        let error = request
            .resolve(RiichiVariant::Sanma)
            .expect_err("invalid sanma options");
        let RuleResolutionError::Validation(errors) = error else {
            panic!("expected validation errors");
        };

        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn official_presets_are_yonma_only_and_revisioned() {
        let sanma = RoomRuleRequest {
            preset: Some(crate::PresetRequest {
                id: "m-league".to_owned(),
                revision: None,
            }),
            overrides: crate::RiichiRuleOverrides::default(),
        }
        .resolve(RiichiVariant::Sanma)
        .expect_err("preset is yonma only");
        assert_eq!(sanma.code(), "rules.preset.variant_mismatch");

        let future_revision = RoomRuleRequest {
            preset: Some(crate::PresetRequest {
                id: "m-league".to_owned(),
                revision: Some(2),
            }),
            overrides: crate::RiichiRuleOverrides::default(),
        }
        .resolve(RiichiVariant::Yonma)
        .expect_err("revision is not defined");
        assert_eq!(future_revision.code(), "rules.preset.unsupported_revision");
    }
}
