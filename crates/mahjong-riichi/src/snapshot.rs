use std::num::NonZeroU32;

use mahjong_core::{EngineVersion, PresetId, PresetRef, RuleSetId, RuleSnapshot};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    RIICHI_ENGINE_VERSION, ResolvedRiichiRules, RiichiRules, RiichiVariant, RoomRuleRequest,
    RuleResolutionError, ValidationErrors,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiichiRuleSnapshot {
    inner: RuleSnapshot<RiichiRules>,
}

impl RiichiRuleSnapshot {
    pub fn try_new(
        rules: RiichiRules,
        preset: Option<PresetRef>,
    ) -> Result<Self, ValidationErrors> {
        rules.validate()?;
        Ok(Self::new_validated(rules, preset))
    }

    #[must_use]
    pub fn from_resolved(resolved: ResolvedRiichiRules) -> Self {
        let (rules, preset) = resolved.into_parts();
        Self::new_validated(rules, preset)
    }

    fn new_validated(rules: RiichiRules, preset: Option<PresetRef>) -> Self {
        let rule_set_id =
            RuleSetId::parse(rules.variant.rule_set_id()).expect("built-in rule set ID is valid");
        Self {
            inner: RuleSnapshot::new(rule_set_id, RIICHI_ENGINE_VERSION, preset, rules),
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.inner.schema_version()
    }

    #[must_use]
    pub const fn rule_set_id(&self) -> &RuleSetId {
        self.inner.rule_set_id()
    }

    #[must_use]
    pub const fn engine_version(&self) -> EngineVersion {
        self.inner.engine_version()
    }

    #[must_use]
    pub const fn preset(&self) -> Option<&PresetRef> {
        self.inner.preset()
    }

    #[must_use]
    pub const fn rules(&self) -> &RiichiRules {
        self.inner.config()
    }

    #[must_use]
    pub fn into_rules(self) -> RiichiRules {
        self.inner.into_config()
    }
}

impl RoomRuleRequest {
    pub fn resolve_snapshot(
        self,
        variant: RiichiVariant,
    ) -> Result<RiichiRuleSnapshot, RuleResolutionError> {
        self.resolve(variant).map(RiichiRuleSnapshot::from_resolved)
    }
}

#[derive(Serialize)]
struct SnapshotWrite<'a> {
    schema_version: u16,
    rule_set_id: &'a str,
    engine_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preset: Option<PresetWrite<'a>>,
    config: &'a RiichiRules,
}

#[derive(Serialize)]
struct PresetWrite<'a> {
    id: &'a str,
    revision: u32,
}

impl Serialize for RiichiRuleSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let preset = self.preset().map(|preset| PresetWrite {
            id: preset.id().as_str(),
            revision: preset.revision().get(),
        });
        SnapshotWrite {
            schema_version: self.schema_version(),
            rule_set_id: self.rule_set_id().as_str(),
            engine_version: self.engine_version().to_string(),
            preset,
            config: self.rules(),
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRead {
    schema_version: u16,
    rule_set_id: String,
    engine_version: String,
    #[serde(default)]
    preset: Option<PresetRead>,
    config: RiichiRules,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetRead {
    id: String,
    revision: u32,
}

impl<'de> Deserialize<'de> for RiichiRuleSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = SnapshotRead::deserialize(deserializer)?;
        if value.schema_version != 1 {
            return Err(D::Error::custom(format_args!(
                "unsupported rule snapshot schema version {}",
                value.schema_version
            )));
        }

        let rule_set_id = RuleSetId::parse(value.rule_set_id).map_err(D::Error::custom)?;
        let expected_rule_set_id = value.config.variant.rule_set_id();
        if rule_set_id.as_str() != expected_rule_set_id {
            return Err(D::Error::custom(format_args!(
                "rule_set_id must be {expected_rule_set_id} for {:?}",
                value.config.variant
            )));
        }

        let engine_version = value
            .engine_version
            .parse::<EngineVersion>()
            .map_err(D::Error::custom)?;
        let preset = value
            .preset
            .map(|preset| {
                let id = PresetId::parse(preset.id).map_err(D::Error::custom)?;
                let revision = NonZeroU32::new(preset.revision)
                    .ok_or_else(|| D::Error::custom("preset revision must be non-zero"))?;
                Ok(PresetRef::new(id, revision))
            })
            .transpose()?;

        value.config.validate().map_err(|errors| {
            let codes = errors
                .violations()
                .iter()
                .map(crate::RuleViolation::code)
                .collect::<Vec<_>>()
                .join(", ");
            D::Error::custom(format_args!("invalid rule config: {codes}"))
        })?;

        Ok(Self {
            inner: RuleSnapshot::new(rule_set_id, engine_version, preset, value.config),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        PresetRequest, RiichiPreset, RiichiRuleOverrides, RiichiRuleSnapshot, RiichiRules,
        RiichiVariant, RoomRuleRequest,
    };

    const M_LEAGUE_FIXTURE: &str = include_str!("../fixtures/rule-snapshots/m-league-v1.json");

    #[test]
    fn m_league_snapshot_matches_stable_fixture() {
        let snapshot = RoomRuleRequest {
            preset: Some(PresetRequest {
                id: RiichiPreset::MLeague.id().to_owned(),
                revision: Some(1),
            }),
            overrides: RiichiRuleOverrides::default(),
        }
        .resolve_snapshot(RiichiVariant::Yonma)
        .expect("valid snapshot");

        let encoded = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");

        assert_eq!(format!("{encoded}\n"), M_LEAGUE_FIXTURE);
        let decoded: RiichiRuleSnapshot =
            serde_json::from_str(M_LEAGUE_FIXTURE).expect("read snapshot fixture");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn sanma_snapshot_uses_matching_rule_set_id() {
        let snapshot =
            RiichiRuleSnapshot::try_new(RiichiRules::standard(RiichiVariant::Sanma), None)
                .expect("valid sanma");

        assert_eq!(snapshot.rule_set_id().as_str(), "riichi/sanma");
        assert_eq!(snapshot.engine_version().to_string(), "0.1.0");
        assert!(snapshot.preset().is_none());
    }

    #[test]
    fn rejects_unknown_snapshot_fields() {
        let mut value: serde_json::Value =
            serde_json::from_str(M_LEAGUE_FIXTURE).expect("fixture JSON");
        value["created_at"] = json!("2026-07-30T00:00:00Z");

        assert!(serde_json::from_value::<RiichiRuleSnapshot>(value).is_err());
    }

    #[test]
    fn rejects_rule_set_and_config_variant_mismatch() {
        let mut value: serde_json::Value =
            serde_json::from_str(M_LEAGUE_FIXTURE).expect("fixture JSON");
        value["rule_set_id"] = json!("riichi/sanma");

        let error =
            serde_json::from_value::<RiichiRuleSnapshot>(value).expect_err("mismatched rule set");

        assert!(
            error
                .to_string()
                .contains("rule_set_id must be riichi/yonma")
        );
    }

    #[test]
    fn validates_full_config_when_reading_from_storage() {
        let mut value: serde_json::Value =
            serde_json::from_str(M_LEAGUE_FIXTURE).expect("fixture JSON");
        value["config"]["bonuses"]["red_fives"]["man"] = json!(9);

        let error =
            serde_json::from_value::<RiichiRuleSnapshot>(value).expect_err("invalid red count");

        assert!(error.to_string().contains("rules.red_fives.too_many"));
    }

    #[test]
    fn full_snapshot_does_not_depend_on_current_preset_catalog() {
        let mut value: serde_json::Value =
            serde_json::from_str(M_LEAGUE_FIXTURE).expect("fixture JSON");
        value["preset"]["id"] = json!("retired-preset");
        value["preset"]["revision"] = json!(37);

        let snapshot: RiichiRuleSnapshot =
            serde_json::from_value(value).expect("complete historical snapshot");

        assert_eq!(
            snapshot.preset().expect("preset metadata").id().as_str(),
            "retired-preset"
        );
        assert_eq!(
            snapshot.preset().expect("preset metadata").revision().get(),
            37
        );
        assert_eq!(snapshot.rules(), &RiichiPreset::MLeague.rules());
    }
}
