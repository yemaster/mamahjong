//! 房间规则请求：预设 + 覆盖 → 展开成完整配置 → 校验。
//!
//! 每一层都是 `deny_unknown_fields`，写错字段名会直接被拒，而不是被当成「没设」。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{PresetId, PresetRef};
use serde::{Deserialize, Serialize};

use crate::config::{ImpactMode, ImpactRules, ThinkingTimeRules};
use crate::preset::ImpactPreset;
use crate::validation::ValidationErrors;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresetRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactRoomRuleRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetRequest>,
    #[serde(default)]
    pub overrides: ImpactRuleOverrides,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactRuleOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ImpactMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_rules: Option<MatchRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kan: Option<KanRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special: Option<SpecialRuleOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_in: Option<AllInRuleOverrides>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRuleOverrides {
    pub thinking_time: Option<ThinkingTimeRules>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KanRuleOverrides {
    pub added_kan_single_payer: Option<bool>,
    pub indicator_pon_counts_as_kan: Option<bool>,
    pub first_round_repeat_discard: Option<bool>,
    pub four_identical_discards_as_kan: Option<bool>,
    pub pon_with_few_tiles_as_kan: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialRuleOverrides {
    pub seven_gaps: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllInRuleOverrides {
    pub eleven_honor_streak: Option<bool>,
    pub all_honors: Option<bool>,
    pub pure_flush_no_joker: Option<bool>,
    pub single_wait: Option<bool>,
    pub three_kans: Option<bool>,
    pub four_jokers: Option<bool>,
    pub pure_seven_pairs: Option<bool>,
    pub last_tile: Option<bool>,
    pub blessing: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImpactRules {
    rules: ImpactRules,
    preset: Option<PresetRef>,
}

impl ResolvedImpactRules {
    #[must_use]
    pub const fn rules(&self) -> &ImpactRules {
        &self.rules
    }

    #[must_use]
    pub const fn preset(&self) -> Option<&PresetRef> {
        self.preset.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (ImpactRules, Option<PresetRef>) {
        (self.rules, self.preset)
    }
}

impl ImpactRoomRuleRequest {
    pub fn resolve(self) -> Result<ResolvedImpactRules, RuleResolutionError> {
        let (mut rules, preset) = match self.preset {
            Some(request) => resolve_preset(request)?,
            None => (ImpactRules::standard(), None),
        };

        self.overrides.apply_to(&mut rules);
        rules.validate().map_err(RuleResolutionError::Validation)?;

        Ok(ResolvedImpactRules { rules, preset })
    }
}

impl ImpactRuleOverrides {
    fn apply_to(self, rules: &mut ImpactRules) {
        apply_if_some(&mut rules.mode, self.mode);

        if let Some(overrides) = self.match_rules {
            apply_if_some(
                &mut rules.match_rules.thinking_time,
                overrides.thinking_time,
            );
        }

        if let Some(overrides) = self.kan {
            apply_if_some(
                &mut rules.kan.added_kan_single_payer,
                overrides.added_kan_single_payer,
            );
            apply_if_some(
                &mut rules.kan.indicator_pon_counts_as_kan,
                overrides.indicator_pon_counts_as_kan,
            );
            apply_if_some(
                &mut rules.kan.first_round_repeat_discard,
                overrides.first_round_repeat_discard,
            );
            apply_if_some(
                &mut rules.kan.four_identical_discards_as_kan,
                overrides.four_identical_discards_as_kan,
            );
            apply_if_some(
                &mut rules.kan.pon_with_few_tiles_as_kan,
                overrides.pon_with_few_tiles_as_kan,
            );
        }

        if let Some(overrides) = self.special {
            apply_if_some(&mut rules.special.seven_gaps, overrides.seven_gaps);
        }

        if let Some(overrides) = self.all_in {
            apply_if_some(
                &mut rules.all_in.eleven_honor_streak,
                overrides.eleven_honor_streak,
            );
            apply_if_some(&mut rules.all_in.all_honors, overrides.all_honors);
            apply_if_some(
                &mut rules.all_in.pure_flush_no_joker,
                overrides.pure_flush_no_joker,
            );
            apply_if_some(&mut rules.all_in.single_wait, overrides.single_wait);
            apply_if_some(&mut rules.all_in.three_kans, overrides.three_kans);
            apply_if_some(&mut rules.all_in.four_jokers, overrides.four_jokers);
            apply_if_some(
                &mut rules.all_in.pure_seven_pairs,
                overrides.pure_seven_pairs,
            );
            apply_if_some(&mut rules.all_in.last_tile, overrides.last_tile);
            apply_if_some(&mut rules.all_in.blessing, overrides.blessing);
        }
    }
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

fn resolve_preset(
    request: PresetRequest,
) -> Result<(ImpactRules, Option<PresetRef>), RuleResolutionError> {
    if PresetId::parse(request.id.as_str()).is_err() {
        return Err(RuleResolutionError::InvalidPresetId { id: request.id });
    }

    let preset =
        ImpactPreset::find(&request.id).ok_or_else(|| RuleResolutionError::UnknownPreset {
            id: request.id.clone(),
        })?;

    let requested_revision = request.revision.unwrap_or(preset.revision().get());
    if requested_revision != preset.revision().get() {
        return Err(RuleResolutionError::UnsupportedPresetRevision {
            id: request.id,
            revision: requested_revision,
        });
    }

    Ok((preset.rules(), Some(preset.preset_ref())))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleResolutionError {
    InvalidPresetId { id: String },
    UnknownPreset { id: String },
    UnsupportedPresetRevision { id: String, revision: u32 },
    Validation(ValidationErrors),
}

impl RuleResolutionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidPresetId { .. } => "rules.preset.invalid_id",
            Self::UnknownPreset { .. } => "rules.preset.unknown",
            Self::UnsupportedPresetRevision { .. } => "rules.preset.unsupported_revision",
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

    use super::{ImpactRoomRuleRequest, PresetRequest, RuleResolutionError};

    #[test]
    fn empty_request_resolves_to_the_standard_defaults() {
        let resolved = ImpactRoomRuleRequest::default()
            .resolve()
            .expect("standard defaults");

        assert!(resolved.rules().kan.added_kan_single_payer);
        assert!(!resolved.rules().special.seven_gaps);
        assert!(resolved.preset().is_none());
    }

    #[test]
    fn applies_partial_overrides_on_top_of_preset() {
        let request: ImpactRoomRuleRequest = serde_json::from_value(json!({
            "preset": {"id": "standard", "revision": 1},
            "overrides": {
                "kan": {"added_kan_single_payer": false},
                "special": {"seven_gaps": true},
                "all_in": {"three_kans": false}
            }
        }))
        .expect("valid request");

        let resolved = request.resolve().expect("valid overrides");

        assert!(!resolved.rules().kan.added_kan_single_payer);
        assert!(resolved.rules().kan.indicator_pon_counts_as_kan);
        assert!(resolved.rules().special.seven_gaps);
        assert!(!resolved.rules().all_in.three_kans);
        assert!(resolved.rules().all_in.four_jokers);
        assert_eq!(resolved.preset().expect("preset").id().as_str(), "standard");
    }

    #[test]
    fn rejects_unknown_fields_at_every_override_level() {
        let top_level = serde_json::from_value::<ImpactRoomRuleRequest>(json!({
            "overrides": {},
            "variant": "yonma"
        }));
        let nested = serde_json::from_value::<ImpactRoomRuleRequest>(json!({
            "overrides": {"kan": {"added_kan": true}}
        }));

        assert!(top_level.is_err());
        assert!(nested.is_err());
    }

    #[test]
    fn validation_runs_after_all_overrides_are_applied() {
        let request: ImpactRoomRuleRequest = serde_json::from_value(json!({
            "overrides": {
                "match_rules": {"thinking_time": {"base_seconds": 7, "reserve_seconds": 3}}
            }
        }))
        .expect("structurally valid request");

        let error = request.resolve().expect_err("unsupported thinking time");

        assert_eq!(error.code(), "request.invalid_rule_config");
        assert!(matches!(error, RuleResolutionError::Validation(_)));
    }

    #[test]
    fn unknown_presets_and_revisions_are_rejected() {
        let unknown = ImpactRoomRuleRequest {
            preset: Some(PresetRequest {
                id: "m-league".to_owned(),
                revision: None,
            }),
            overrides: Default::default(),
        }
        .resolve()
        .expect_err("impact has no such preset");
        assert_eq!(unknown.code(), "rules.preset.unknown");

        let future_revision = ImpactRoomRuleRequest {
            preset: Some(PresetRequest {
                id: "standard".to_owned(),
                revision: Some(2),
            }),
            overrides: Default::default(),
        }
        .resolve()
        .expect_err("revision is not defined");
        assert_eq!(future_revision.code(), "rules.preset.unsupported_revision");
    }
}
