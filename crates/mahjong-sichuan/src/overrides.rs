//! 房间规则请求：预设 + 覆盖 → 展开成完整配置 → 校验。
//!
//! 每一层都是 `deny_unknown_fields`，写错字段名会直接被拒，而不是被当成「没设」。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{PresetId, PresetRef};
use serde::{Deserialize, Serialize};

use crate::config::{SichuanRules, ThinkingTimeRules};
use crate::preset::SichuanPreset;
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
pub struct SichuanRoomRuleRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetRequest>,
    #[serde(default)]
    pub overrides: SichuanRuleOverrides,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SichuanRuleOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_rules: Option<MatchRuleOverrides>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRuleOverrides {
    pub thinking_time: Option<ThinkingTimeRules>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSichuanRules {
    rules: SichuanRules,
    preset: Option<PresetRef>,
}

impl ResolvedSichuanRules {
    #[must_use]
    pub const fn rules(&self) -> &SichuanRules {
        &self.rules
    }

    #[must_use]
    pub const fn preset(&self) -> Option<&PresetRef> {
        self.preset.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (SichuanRules, Option<PresetRef>) {
        (self.rules, self.preset)
    }
}

impl SichuanRoomRuleRequest {
    pub fn resolve(self) -> Result<ResolvedSichuanRules, RuleResolutionError> {
        let (mut rules, preset) = match self.preset {
            Some(request) => resolve_preset(request)?,
            None => (SichuanRules::standard(), None),
        };

        self.overrides.apply_to(&mut rules);
        rules.validate().map_err(RuleResolutionError::Validation)?;

        Ok(ResolvedSichuanRules { rules, preset })
    }
}

impl SichuanRuleOverrides {
    fn apply_to(self, rules: &mut SichuanRules) {
        if let Some(overrides) = self.match_rules {
            apply_if_some(
                &mut rules.match_rules.thinking_time,
                overrides.thinking_time,
            );
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
) -> Result<(SichuanRules, Option<PresetRef>), RuleResolutionError> {
    if PresetId::parse(request.id.as_str()).is_err() {
        return Err(RuleResolutionError::InvalidPresetId { id: request.id });
    }

    let preset =
        SichuanPreset::find(&request.id).ok_or_else(|| RuleResolutionError::UnknownPreset {
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

    use super::{PresetRequest, RuleResolutionError, SichuanRoomRuleRequest};

    #[test]
    fn empty_request_resolves_to_the_standard_defaults() {
        let resolved = SichuanRoomRuleRequest::default()
            .resolve()
            .expect("standard defaults");

        assert_eq!(resolved.rules().match_rules.thinking_time.base_seconds, 5);
        assert!(resolved.preset().is_none());
    }

    #[test]
    fn applies_partial_overrides_on_top_of_preset() {
        let request: SichuanRoomRuleRequest = serde_json::from_value(json!({
            "preset": {"id": "standard", "revision": 1},
            "overrides": {
                "match_rules": {"thinking_time": {"base_seconds": 15, "reserve_seconds": 60}}
            }
        }))
        .expect("valid request");

        let resolved = request.resolve().expect("valid overrides");

        assert_eq!(resolved.rules().match_rules.thinking_time.base_seconds, 15);
        assert_eq!(
            resolved.rules().match_rules.thinking_time.reserve_seconds,
            60
        );
        assert_eq!(resolved.preset().expect("preset").id().as_str(), "standard");
    }

    #[test]
    fn rejects_unknown_fields_at_every_override_level() {
        let top_level = serde_json::from_value::<SichuanRoomRuleRequest>(json!({
            "overrides": {},
            "variant": "yonma"
        }));
        let nested = serde_json::from_value::<SichuanRoomRuleRequest>(json!({
            "overrides": {"match_rules": {"thinking": {"base_seconds": 5}}}
        }));

        assert!(top_level.is_err());
        assert!(nested.is_err());
    }

    #[test]
    fn validation_runs_after_all_overrides_are_applied() {
        let request: SichuanRoomRuleRequest = serde_json::from_value(json!({
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
        let unknown = SichuanRoomRuleRequest {
            preset: Some(PresetRequest {
                id: "m-league".to_owned(),
                revision: None,
            }),
            overrides: Default::default(),
        }
        .resolve()
        .expect_err("sichuan has no such preset");
        assert_eq!(unknown.code(), "rules.preset.unknown");

        let future_revision = SichuanRoomRuleRequest {
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
