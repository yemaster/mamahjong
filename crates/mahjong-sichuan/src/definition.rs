//! 规则定义：把 `sichuan/yonma` 注册进 `mahjong-core` 的规则目录。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{EngineVersion, RuleDefinition, RuleDescriptor, RuleSetId, SeatCount};

use crate::config::{SEAT_COUNT, SichuanRules};
use crate::validation::ValidationErrors;

pub const SICHUAN_ENGINE_VERSION: EngineVersion = EngineVersion::new(0, 1, 0);

/// 规则集 ID。四川麻将只有四人一种。
pub const SICHUAN_RULE_SET_ID: &str = "sichuan/yonma";

#[derive(Debug)]
pub struct SichuanRuleDefinition {
    descriptor: RuleDescriptor,
}

impl SichuanRuleDefinition {
    #[must_use]
    pub fn new() -> Self {
        let seat_count = SeatCount::new(SEAT_COUNT).expect("sichuan is always a four-player table");
        let descriptor = RuleDescriptor::new(
            RuleSetId::parse(SICHUAN_RULE_SET_ID).expect("built-in rule set ID is valid"),
            SICHUAN_ENGINE_VERSION,
            "四川麻将",
            [seat_count],
        )
        .expect("built-in rule descriptor is valid");

        Self { descriptor }
    }
}

impl Default for SichuanRuleDefinition {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleDefinition for SichuanRuleDefinition {
    type Config = SichuanRules;
    type ValidationError = SichuanDefinitionError;

    fn descriptor(&self) -> &RuleDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &Self::Config) -> Result<(), Self::ValidationError> {
        config
            .validate()
            .map_err(SichuanDefinitionError::Validation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SichuanDefinitionError {
    Validation(ValidationErrors),
}

impl Display for SichuanDefinitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(errors) => Display::fmt(errors, formatter),
        }
    }
}

impl Error for SichuanDefinitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(errors) => Some(errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use mahjong_core::RuleDefinition;

    use super::SichuanRuleDefinition;
    use crate::config::SichuanRules;

    #[test]
    fn descriptor_is_a_four_player_sichuan_table() {
        let definition = SichuanRuleDefinition::new();

        assert_eq!(definition.descriptor().id().as_str(), "sichuan/yonma");
        assert_eq!(definition.descriptor().display_name(), "四川麻将");
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
        let definition = SichuanRuleDefinition::new();
        definition
            .validate_config(&SichuanRules::standard())
            .expect("standard config");

        let mut rules = SichuanRules::standard();
        rules.match_rules.thinking_time.base_seconds = 3;

        let error = definition
            .validate_config(&rules)
            .expect_err("unsupported thinking time");

        assert!(error.to_string().contains("1 violation"));
    }
}
