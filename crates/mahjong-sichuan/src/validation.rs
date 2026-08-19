//! 配置校验。一次跑完所有检查，把违规项一起攒出来再返回。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::config::SichuanRules;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleViolation {
    code: &'static str,
    field: &'static str,
    message: String,
}

impl RuleViolation {
    fn new(code: &'static str, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            field,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationErrors {
    violations: Box<[RuleViolation]>,
}

impl ValidationErrors {
    fn new(violations: Vec<RuleViolation>) -> Self {
        debug_assert!(!violations.is_empty());
        Self {
            violations: violations.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn violations(&self) -> &[RuleViolation] {
        &self.violations
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.violations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
    }
}

impl Display for ValidationErrors {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "rule configuration contains {} violation(s)",
            self.violations.len()
        )
    }
}

impl Error for ValidationErrors {}

impl SichuanRules {
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut violations = Vec::new();

        let time = self.match_rules.thinking_time;
        if !matches!(
            (time.base_seconds, time.reserve_seconds),
            (5, 0) | (5, 20) | (5, 60) | (15, 60)
        ) {
            violations.push(RuleViolation::new(
                "rules.thinking_time.unsupported",
                "match_rules.thinking_time",
                "must be one of 5+0, 5+20, 5+60, or 15+60",
            ));
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors::new(violations))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SichuanRules;

    #[test]
    fn standard_rules_validate() {
        SichuanRules::standard().validate().expect("standard rules");
    }

    #[test]
    fn thinking_time_accepts_only_the_room_presets() {
        let mut rules = SichuanRules::standard();
        rules.match_rules.thinking_time.base_seconds = 10;

        let errors = rules.validate().expect_err("unsupported thinking time");

        assert!(errors.violations().iter().any(|violation| {
            violation.code() == "rules.thinking_time.unsupported"
                && violation.field() == "match_rules.thinking_time"
        }));
    }

    #[test]
    fn every_supported_thinking_time_validates() {
        for (base, reserve) in [(5, 0), (5, 20), (5, 60), (15, 60)] {
            let mut rules = SichuanRules::standard();
            rules.match_rules.thinking_time.base_seconds = base;
            rules.match_rules.thinking_time.reserve_seconds = reserve;
            rules
                .validate()
                .unwrap_or_else(|_| panic!("{base}+{reserve} should be accepted"));
        }
    }
}
