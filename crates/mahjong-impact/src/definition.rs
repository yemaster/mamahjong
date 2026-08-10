//! 规则定义：把 `impact/yonma` 注册进 `mahjong-core` 的规则目录。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{EngineVersion, RuleDefinition, RuleDescriptor, RuleSetId, SeatCount};

use crate::config::{ImpactRules, SEAT_COUNT};
use crate::validation::ValidationErrors;

pub const IMPACT_ENGINE_VERSION: EngineVersion = EngineVersion::new(0, 1, 0);

/// 规则集 ID。冲击麻将只有四人一种。
pub const IMPACT_RULE_SET_ID: &str = "impact/yonma";

#[derive(Debug)]
pub struct ImpactRuleDefinition {
    descriptor: RuleDescriptor,
}

impl ImpactRuleDefinition {
    #[must_use]
    pub fn new() -> Self {
        let seat_count = SeatCount::new(SEAT_COUNT).expect("impact is always a four-player table");
        let descriptor = RuleDescriptor::new(
            RuleSetId::parse(IMPACT_RULE_SET_ID).expect("built-in rule set ID is valid"),
            IMPACT_ENGINE_VERSION,
            "冲击麻将",
            [seat_count],
        )
        .expect("built-in rule descriptor is valid");

        Self { descriptor }
    }
}

impl Default for ImpactRuleDefinition {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleDefinition for ImpactRuleDefinition {
    type Config = ImpactRules;
    type ValidationError = ImpactDefinitionError;

    fn descriptor(&self) -> &RuleDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &Self::Config) -> Result<(), Self::ValidationError> {
        config.validate().map_err(ImpactDefinitionError::Validation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImpactDefinitionError {
    Validation(ValidationErrors),
}

impl Display for ImpactDefinitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(errors) => Display::fmt(errors, formatter),
        }
    }
}

impl Error for ImpactDefinitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(errors) => Some(errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use mahjong_core::RuleDefinition;

    use super::ImpactRuleDefinition;
    use crate::config::ImpactRules;

    #[test]
    fn descriptor_is_a_four_player_impact_table() {
        let definition = ImpactRuleDefinition::new();

        assert_eq!(definition.descriptor().id().as_str(), "impact/yonma");
        assert_eq!(definition.descriptor().display_name(), "冲击麻将");
        assert_eq!(
            definition.descriptor().supported_seat_counts()[0].value(),
            4
        );
        assert_eq!(
            definition.descriptor().engine_version().to_string(),
            "0.1.0"
        );
    }

    #[test]
    fn definition_delegates_config_validation() {
        let definition = ImpactRuleDefinition::new();
        definition
            .validate_config(&ImpactRules::standard())
            .expect("standard config");

        let mut rules = ImpactRules::standard();
        rules.match_rules.thinking_time.base_seconds = 3;

        let error = definition
            .validate_config(&rules)
            .expect_err("unsupported thinking time");

        assert!(error.to_string().contains("1 violation"));
    }
}
