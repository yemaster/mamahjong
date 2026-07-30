use std::error::Error;
use std::fmt::{self, Display, Formatter};

use mahjong_core::{EngineVersion, RuleDefinition, RuleDescriptor, RuleSetId, SeatCount};

use crate::{RiichiRules, RiichiVariant, ValidationErrors};

pub const RIICHI_ENGINE_VERSION: EngineVersion = EngineVersion::new(0, 1, 0);

#[derive(Debug)]
pub struct RiichiRuleDefinition {
    variant: RiichiVariant,
    descriptor: RuleDescriptor,
}

impl RiichiRuleDefinition {
    #[must_use]
    pub fn new(variant: RiichiVariant) -> Self {
        let display_name = match variant {
            RiichiVariant::Yonma => "四人立直麻将",
            RiichiVariant::Sanma => "三人立直麻将",
        };
        let seat_count = SeatCount::new(variant.seat_count().value())
            .expect("riichi variants always have a supported seat count");
        let descriptor = RuleDescriptor::new(
            RuleSetId::parse(variant.rule_set_id()).expect("built-in rule set ID is valid"),
            RIICHI_ENGINE_VERSION,
            display_name,
            [seat_count],
        )
        .expect("built-in rule descriptor is valid");

        Self {
            variant,
            descriptor,
        }
    }

    #[must_use]
    pub const fn variant(&self) -> RiichiVariant {
        self.variant
    }
}

impl RuleDefinition for RiichiRuleDefinition {
    type Config = RiichiRules;
    type ValidationError = RiichiDefinitionError;

    fn descriptor(&self) -> &RuleDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &Self::Config) -> Result<(), Self::ValidationError> {
        if config.variant != self.variant {
            return Err(RiichiDefinitionError::VariantMismatch {
                expected: self.variant,
                actual: config.variant,
            });
        }
        config.validate().map_err(RiichiDefinitionError::Validation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiichiDefinitionError {
    VariantMismatch {
        expected: RiichiVariant,
        actual: RiichiVariant,
    },
    Validation(ValidationErrors),
}

impl Display for RiichiDefinitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariantMismatch { expected, actual } => {
                write!(
                    formatter,
                    "rule definition for {expected:?} cannot validate {actual:?} config"
                )
            }
            Self::Validation(errors) => Display::fmt(errors, formatter),
        }
    }
}

impl Error for RiichiDefinitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(errors) => Some(errors),
            Self::VariantMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use mahjong_core::RuleDefinition;

    use crate::{RiichiRuleDefinition, RiichiRules, RiichiVariant};

    #[test]
    fn exposes_one_descriptor_per_variant() {
        let yonma = RiichiRuleDefinition::new(RiichiVariant::Yonma);
        let sanma = RiichiRuleDefinition::new(RiichiVariant::Sanma);

        assert_eq!(yonma.descriptor().id().as_str(), "riichi/yonma");
        assert_eq!(yonma.descriptor().supported_seat_counts()[0].value(), 4);
        assert_eq!(sanma.descriptor().id().as_str(), "riichi/sanma");
        assert_eq!(sanma.descriptor().supported_seat_counts()[0].value(), 3);
        assert_eq!(yonma.descriptor().engine_version().to_string(), "0.1.0");
    }

    #[test]
    fn definition_guards_variant_boundary() {
        let definition = RiichiRuleDefinition::new(RiichiVariant::Yonma);
        let sanma = RiichiRules::standard(RiichiVariant::Sanma);

        let error = definition
            .validate_config(&sanma)
            .expect_err("variant mismatch");

        assert!(error.to_string().contains("cannot validate Sanma"));
    }

    #[test]
    fn definition_delegates_complete_config_validation() {
        let definition = RiichiRuleDefinition::new(RiichiVariant::Yonma);
        let mut rules = RiichiRules::default();
        rules.match_rules.initial_points = 123;

        let error = definition
            .validate_config(&rules)
            .expect_err("invalid points");

        assert!(error.to_string().contains("2 violation"));
    }
}
