use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{PlacementUma, RiichiRules, RiichiVariant, Suit};

const MIN_CONFIGURED_POINTS: u32 = 1_000;
const MAX_CONFIGURED_POINTS: u32 = 1_000_000;
const MAX_NOTEN_PAYMENT: u32 = 100_000;
const POINT_UNIT: u32 = 1_000;
const MAX_RED_FIVES_PER_SUIT: u8 = 4;

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

impl RiichiRules {
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut violations = Vec::new();

        validate_configured_points(
            self.match_rules.initial_points,
            "match_rules.initial_points",
            &mut violations,
        );
        validate_configured_points(
            self.match_rules.return_points,
            "match_rules.return_points",
            &mut violations,
        );
        validate_configured_points(
            self.match_rules.first_place_required_points,
            "match_rules.first_place_required_points",
            &mut violations,
        );
        validate_thinking_time(self, &mut violations);
        validate_noten_payment(self.settlement.noten_payment, self.variant, &mut violations);
        validate_red_fives(self, &mut violations);
        validate_variant_options(self, &mut violations);
        validate_uma(self, &mut violations);

        if violations.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors::new(violations))
        }
    }
}

fn validate_thinking_time(rules: &RiichiRules, violations: &mut Vec<RuleViolation>) {
    let time = rules.match_rules.thinking_time;
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
}

fn validate_configured_points(
    points: u32,
    field: &'static str,
    violations: &mut Vec<RuleViolation>,
) {
    if !(MIN_CONFIGURED_POINTS..=MAX_CONFIGURED_POINTS).contains(&points) {
        violations.push(RuleViolation::new(
            "rules.points.out_of_range",
            field,
            format!(
                "must be between {MIN_CONFIGURED_POINTS} and {MAX_CONFIGURED_POINTS}, got {points}"
            ),
        ));
    }
    if !is_thousand_aligned(points) {
        violations.push(RuleViolation::new(
            "rules.points.not_thousand_aligned",
            field,
            format!("must be a multiple of {POINT_UNIT}, got {points}"),
        ));
    }
}

fn validate_noten_payment(
    points: u32,
    variant: RiichiVariant,
    violations: &mut Vec<RuleViolation>,
) {
    const FIELD: &str = "settlement.noten_payment";

    if points > MAX_NOTEN_PAYMENT {
        violations.push(RuleViolation::new(
            "rules.noten_payment.out_of_range",
            FIELD,
            format!("must be at most {MAX_NOTEN_PAYMENT}, got {points}"),
        ));
    }
    if !is_thousand_aligned(points) {
        violations.push(RuleViolation::new(
            "rules.points.not_thousand_aligned",
            FIELD,
            format!("must be a multiple of {POINT_UNIT}, got {points}"),
        ));
    }
    let seat_count = u32::from(variant.seat_count().value());
    if points > 0
        && (1..seat_count).any(|count| {
            !is_evenly_divisible(points, count) || !is_evenly_divisible(points, seat_count - count)
        })
    {
        violations.push(RuleViolation::new(
            "rules.noten_payment.indivisible",
            FIELD,
            "must divide evenly for every possible tenpai/noten split",
        ));
    }
}

#[allow(clippy::manual_is_multiple_of)]
const fn is_thousand_aligned(points: u32) -> bool {
    points % POINT_UNIT == 0
}

#[allow(clippy::manual_is_multiple_of)]
const fn is_evenly_divisible(points: u32, divisor: u32) -> bool {
    points % divisor == 0
}

fn validate_red_fives(rules: &RiichiRules, violations: &mut Vec<RuleViolation>) {
    for (suit, field) in [
        (Suit::Man, "bonuses.red_fives.man"),
        (Suit::Pin, "bonuses.red_fives.pin"),
        (Suit::Sou, "bonuses.red_fives.sou"),
    ] {
        let count = rules.bonuses.red_fives.for_suit(suit);
        if count > MAX_RED_FIVES_PER_SUIT {
            violations.push(RuleViolation::new(
                "rules.red_fives.too_many",
                field,
                format!("must be at most {MAX_RED_FIVES_PER_SUIT}, got {count}"),
            ));
        }
    }
}

fn validate_variant_options(rules: &RiichiRules, violations: &mut Vec<RuleViolation>) {
    if !matches!(rules.variant, RiichiVariant::Sanma) {
        return;
    }

    if rules.bonuses.red_fives.for_suit(Suit::Man) != 0 {
        violations.push(RuleViolation::new(
            "rules.sanma.red_man_five",
            "bonuses.red_fives.man",
            "sanma removes five-man, so its red count must be zero",
        ));
    }
    if rules.abortive_draws.four_winds {
        violations.push(RuleViolation::new(
            "rules.sanma.four_winds",
            "abortive_draws.four_winds",
            "four-winds abortive draw is unavailable with three players",
        ));
    }
    if rules.abortive_draws.four_riichi {
        violations.push(RuleViolation::new(
            "rules.sanma.four_riichi",
            "abortive_draws.four_riichi",
            "four-riichi abortive draw is unavailable with three players",
        ));
    }
}

fn validate_uma(rules: &RiichiRules, violations: &mut Vec<RuleViolation>) {
    match &rules.settlement.uma {
        PlacementUma::Fixed { values } => {
            if values.len() != usize::from(rules.variant.seat_count().value()) {
                violations.push(RuleViolation::new(
                    "rules.uma.player_count",
                    "settlement.uma.values",
                    format!(
                        "must contain {} entries, got {}",
                        rules.variant.seat_count().value(),
                        values.len()
                    ),
                ));
            }
            if values.iter().map(|value| i32::from(*value)).sum::<i32>() != 0 {
                violations.push(RuleViolation::new(
                    "rules.uma.not_zero_sum",
                    "settlement.uma.values",
                    "fixed placement points must sum to zero",
                ));
            }
        }
        PlacementUma::JpmlA if matches!(rules.variant, RiichiVariant::Sanma) => {
            violations.push(RuleViolation::new(
                "rules.uma.unsupported_variant",
                "settlement.uma",
                "JPML A placement points are only defined for yonma",
            ));
        }
        PlacementUma::JpmlA => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{PlacementUma, RiichiRules, RiichiVariant};

    #[test]
    fn built_in_defaults_are_valid() {
        RiichiRules::standard(RiichiVariant::Yonma)
            .validate()
            .expect("yonma defaults");
        RiichiRules::standard(RiichiVariant::Sanma)
            .validate()
            .expect("sanma defaults");
    }

    #[test]
    fn thinking_time_accepts_only_the_room_presets() {
        let mut rules = RiichiRules::default();
        rules.match_rules.thinking_time.base_seconds = 10;
        let errors = rules.validate().expect_err("unsupported thinking time");
        assert!(errors.violations().iter().any(|violation| {
            violation.code() == "rules.thinking_time.unsupported"
                && violation.field() == "match_rules.thinking_time"
        }));
    }

    #[test]
    fn noten_payment_must_split_without_seat_order_remainders() {
        let mut yonma = RiichiRules::default();
        yonma.settlement.noten_payment = 1_000;
        let errors = yonma.validate().expect_err("indivisible for one tenpai");
        assert!(
            errors
                .violations()
                .iter()
                .any(|violation| violation.code() == "rules.noten_payment.indivisible")
        );

        let mut sanma = RiichiRules::standard(RiichiVariant::Sanma);
        sanma.settlement.noten_payment = 1_000;
        sanma
            .validate()
            .expect("sanma only divides between one and two");
    }

    #[test]
    fn reports_every_independent_violation_in_one_pass() {
        let mut value =
            serde_json::to_value(RiichiRules::default()).expect("serialize default rules");
        value["variant"] = json!("sanma");
        value["match_rules"]["initial_points"] = json!(501);
        value["bonuses"]["red_fives"]["man"] = json!(5);
        value["abortive_draws"]["four_winds"] = json!(true);
        value["abortive_draws"]["four_riichi"] = json!(true);
        value["settlement"]["uma"]["values"] = json!([30, 10, -20]);
        let rules: RiichiRules = serde_json::from_value(value).expect("structurally valid JSON");

        let errors = rules.validate().expect_err("invalid combination");
        let codes: Vec<_> = errors
            .violations()
            .iter()
            .map(super::RuleViolation::code)
            .collect();

        assert_eq!(errors.len(), 7);
        assert!(codes.contains(&"rules.points.out_of_range"));
        assert!(codes.contains(&"rules.points.not_thousand_aligned"));
        assert!(codes.contains(&"rules.red_fives.too_many"));
        assert!(codes.contains(&"rules.sanma.red_man_five"));
        assert!(codes.contains(&"rules.sanma.four_winds"));
        assert!(codes.contains(&"rules.sanma.four_riichi"));
        assert!(codes.contains(&"rules.uma.not_zero_sum"));
    }

    #[test]
    fn validates_fixed_uma_shape_and_sum_separately() {
        let mut rules = RiichiRules::default();
        rules.settlement.uma = PlacementUma::Fixed {
            values: vec![30, 10, -10],
        };

        let errors = rules.validate().expect_err("missing placement");
        let codes: Vec<_> = errors
            .violations()
            .iter()
            .map(super::RuleViolation::code)
            .collect();

        assert!(codes.contains(&"rules.uma.player_count"));
        assert!(codes.contains(&"rules.uma.not_zero_sum"));
    }

    #[test]
    fn rejects_jpml_a_uma_for_sanma() {
        let mut rules = RiichiRules::standard(RiichiVariant::Sanma);
        rules.settlement.uma = PlacementUma::JpmlA;

        let errors = rules.validate().expect_err("unsupported uma");

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors.violations()[0].code(),
            "rules.uma.unsupported_variant"
        );
    }
}
