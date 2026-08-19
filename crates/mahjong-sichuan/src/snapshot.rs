//! 开局快照：把展开后的完整配置连同 schema 版本、规则集 ID、引擎版本、预设引用一起固化。
//!
//! 序列化形状与立直麻将的快照一致，归档里两套规则可以走同一条读写路径。

use std::num::NonZeroU32;

use mahjong_core::{
    EngineVersion, PresetId, PresetRef, RULE_SNAPSHOT_SCHEMA_VERSION, RuleSetId, RuleSnapshot,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::config::SichuanRules;
use crate::definition::{SICHUAN_ENGINE_VERSION, SICHUAN_RULE_SET_ID};
use crate::overrides::{ResolvedSichuanRules, RuleResolutionError, SichuanRoomRuleRequest};
use crate::validation::{RuleViolation, ValidationErrors};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SichuanRuleSnapshot {
    inner: RuleSnapshot<SichuanRules>,
}

impl SichuanRuleSnapshot {
    pub fn try_new(
        rules: SichuanRules,
        preset: Option<PresetRef>,
    ) -> Result<Self, ValidationErrors> {
        rules.validate()?;
        Ok(Self::new_validated(rules, preset))
    }

    #[must_use]
    pub fn from_resolved(resolved: ResolvedSichuanRules) -> Self {
        let (rules, preset) = resolved.into_parts();
        Self::new_validated(rules, preset)
    }

    fn new_validated(rules: SichuanRules, preset: Option<PresetRef>) -> Self {
        let rule_set_id =
            RuleSetId::parse(SICHUAN_RULE_SET_ID).expect("built-in rule set ID is valid");
        Self {
            inner: RuleSnapshot::new(rule_set_id, SICHUAN_ENGINE_VERSION, preset, rules),
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
    pub const fn rules(&self) -> &SichuanRules {
        self.inner.config()
    }

    #[must_use]
    pub fn into_rules(self) -> SichuanRules {
        self.inner.into_config()
    }
}

impl SichuanRoomRuleRequest {
    pub fn resolve_snapshot(self) -> Result<SichuanRuleSnapshot, RuleResolutionError> {
        self.resolve().map(SichuanRuleSnapshot::from_resolved)
    }
}

#[derive(Serialize)]
struct SnapshotWrite<'a> {
    schema_version: u16,
    rule_set_id: &'a str,
    engine_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preset: Option<PresetWrite<'a>>,
    config: &'a SichuanRules,
}

#[derive(Serialize)]
struct PresetWrite<'a> {
    id: &'a str,
    revision: u32,
}

impl Serialize for SichuanRuleSnapshot {
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
    config: SichuanRules,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetRead {
    id: String,
    revision: u32,
}

impl<'de> Deserialize<'de> for SichuanRuleSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = SnapshotRead::deserialize(deserializer)?;
        if value.schema_version != RULE_SNAPSHOT_SCHEMA_VERSION {
            return Err(D::Error::custom(format_args!(
                "unsupported rule snapshot schema version {}",
                value.schema_version
            )));
        }

        let rule_set_id = RuleSetId::parse(value.rule_set_id).map_err(D::Error::custom)?;
        if rule_set_id.as_str() != SICHUAN_RULE_SET_ID {
            return Err(D::Error::custom(format_args!(
                "rule_set_id must be {SICHUAN_RULE_SET_ID} for sichuan rules"
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
                .map(RuleViolation::code)
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

    use super::SichuanRuleSnapshot;
    use crate::config::SichuanRules;
    use crate::overrides::{PresetRequest, SichuanRoomRuleRequest};

    fn standard_snapshot() -> SichuanRuleSnapshot {
        SichuanRoomRuleRequest {
            preset: Some(PresetRequest {
                id: "standard".to_owned(),
                revision: Some(1),
            }),
            overrides: Default::default(),
        }
        .resolve_snapshot()
        .expect("valid snapshot")
    }

    #[test]
    fn snapshot_records_rule_set_engine_and_preset() {
        let snapshot = standard_snapshot();

        assert_eq!(snapshot.schema_version(), 2);
        assert_eq!(snapshot.rule_set_id().as_str(), "sichuan/yonma");
        assert_eq!(snapshot.engine_version().to_string(), "0.1.0");
        assert_eq!(snapshot.preset().expect("preset").id().as_str(), "standard");
        assert_eq!(snapshot.rules(), &SichuanRules::standard());
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let snapshot = standard_snapshot();
        let encoded = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: SichuanRuleSnapshot = serde_json::from_str(&encoded).expect("deserialize");

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn rejects_unknown_snapshot_fields() {
        let mut value =
            serde_json::to_value(standard_snapshot()).expect("snapshot serializes to JSON");
        value["created_at"] = json!("2026-08-08T00:00:00Z");

        assert!(serde_json::from_value::<SichuanRuleSnapshot>(value).is_err());
    }

    #[test]
    fn rejects_a_foreign_rule_set_id() {
        let mut value = serde_json::to_value(standard_snapshot()).expect("snapshot JSON");
        value["rule_set_id"] = json!("riichi/yonma");

        let error =
            serde_json::from_value::<SichuanRuleSnapshot>(value).expect_err("mismatched rule set");

        assert!(
            error
                .to_string()
                .contains("rule_set_id must be sichuan/yonma")
        );
    }

    #[test]
    fn validates_full_config_when_reading_from_storage() {
        let mut value = serde_json::to_value(standard_snapshot()).expect("snapshot JSON");
        value["config"]["match_rules"]["thinking_time"]["base_seconds"] = json!(3);

        let error = serde_json::from_value::<SichuanRuleSnapshot>(value)
            .expect_err("unsupported thinking time");

        assert!(
            error
                .to_string()
                .contains("rules.thinking_time.unsupported")
        );
    }

    #[test]
    fn historical_snapshots_do_not_depend_on_the_current_preset_catalog() {
        let mut value = serde_json::to_value(standard_snapshot()).expect("snapshot JSON");
        value["preset"]["id"] = json!("retired-preset");
        value["preset"]["revision"] = json!(37);

        let snapshot: SichuanRuleSnapshot =
            serde_json::from_value(value).expect("complete historical snapshot");

        assert_eq!(
            snapshot.preset().expect("preset metadata").id().as_str(),
            "retired-preset"
        );
        assert_eq!(
            snapshot.preset().expect("preset metadata").revision().get(),
            37
        );
        assert_eq!(snapshot.rules(), &SichuanRules::standard());
    }
}
