use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroU32;
use std::str::FromStr;

/// Schema version written into every rule snapshot; readers accept only this value.
pub const RULE_SNAPSHOT_SCHEMA_VERSION: u16 = 2;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleSetId(String);

impl RuleSetId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RuleMetadataError> {
        let value = value.into();
        let mut segments = value.split('/');

        let namespace = segments.next().unwrap_or_default();
        let name = segments.next().unwrap_or_default();
        if segments.next().is_some()
            || !is_kebab_identifier(namespace)
            || !is_kebab_identifier(name)
        {
            return Err(RuleMetadataError::InvalidRuleSetId);
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RuleSetId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RuleSetId {
    type Err = RuleMetadataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EngineVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl EngineVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    #[must_use]
    pub const fn patch(self) -> u16 {
        self.patch
    }
}

impl Display for EngineVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for EngineVersion {
    type Err = EngineVersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('.');
        let major = parse_version_part(parts.next())?;
        let minor = parse_version_part(parts.next())?;
        let patch = parse_version_part(parts.next())?;
        if parts.next().is_some() {
            return Err(EngineVersionParseError);
        }

        Ok(Self::new(major, minor, patch))
    }
}

fn parse_version_part(part: Option<&str>) -> Result<u16, EngineVersionParseError> {
    let part = part.ok_or(EngineVersionParseError)?;
    if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
        return Err(EngineVersionParseError);
    }
    part.parse().map_err(|_| EngineVersionParseError)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineVersionParseError;

impl Display for EngineVersionParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("engine version must contain three u16 decimal components")
    }
}

impl Error for EngineVersionParseError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PresetId(String);

impl PresetId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RuleMetadataError> {
        let value = value.into();
        if !is_kebab_identifier(&value) {
            return Err(RuleMetadataError::InvalidPresetId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresetRef {
    id: PresetId,
    revision: NonZeroU32,
}

impl PresetRef {
    #[must_use]
    pub const fn new(id: PresetId, revision: NonZeroU32) -> Self {
        Self { id, revision }
    }

    #[must_use]
    pub const fn id(&self) -> &PresetId {
        &self.id
    }

    #[must_use]
    pub const fn revision(&self) -> NonZeroU32 {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SeatCount(u8);

impl SeatCount {
    pub const MIN: u8 = 2;
    pub const MAX: u8 = 4;

    pub const fn new(value: u8) -> Result<Self, RuleMetadataError> {
        if value >= Self::MIN && value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(RuleMetadataError::InvalidSeatCount)
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDescriptor {
    id: RuleSetId,
    engine_version: EngineVersion,
    display_name: String,
    supported_seat_counts: Vec<SeatCount>,
}

impl RuleDescriptor {
    pub fn new(
        id: RuleSetId,
        engine_version: EngineVersion,
        display_name: impl Into<String>,
        supported_seat_counts: impl IntoIterator<Item = SeatCount>,
    ) -> Result<Self, RuleMetadataError> {
        let display_name = display_name.into();
        if display_name.is_empty()
            || display_name.chars().count() > 80
            || display_name.chars().any(char::is_control)
        {
            return Err(RuleMetadataError::InvalidDisplayName);
        }

        let mut supported_seat_counts: Vec<_> = supported_seat_counts.into_iter().collect();
        supported_seat_counts.sort_unstable();
        supported_seat_counts.dedup();
        if supported_seat_counts.is_empty() {
            return Err(RuleMetadataError::MissingSeatCount);
        }

        Ok(Self {
            id,
            engine_version,
            display_name,
            supported_seat_counts,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &RuleSetId {
        &self.id
    }

    #[must_use]
    pub const fn engine_version(&self) -> EngineVersion {
        self.engine_version
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn supported_seat_counts(&self) -> &[SeatCount] {
        &self.supported_seat_counts
    }
}

pub trait RuleConfig: Clone + Debug + Eq + Send + Sync + 'static {}

impl<T> RuleConfig for T where T: Clone + Debug + Eq + Send + Sync + 'static {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleSnapshot<C: RuleConfig> {
    schema_version: u16,
    rule_set_id: RuleSetId,
    engine_version: EngineVersion,
    preset: Option<PresetRef>,
    config: C,
}

impl<C: RuleConfig> RuleSnapshot<C> {
    #[must_use]
    pub const fn new(
        rule_set_id: RuleSetId,
        engine_version: EngineVersion,
        preset: Option<PresetRef>,
        config: C,
    ) -> Self {
        Self {
            schema_version: RULE_SNAPSHOT_SCHEMA_VERSION,
            rule_set_id,
            engine_version,
            preset,
            config,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub const fn rule_set_id(&self) -> &RuleSetId {
        &self.rule_set_id
    }

    #[must_use]
    pub const fn engine_version(&self) -> EngineVersion {
        self.engine_version
    }

    #[must_use]
    pub const fn preset(&self) -> Option<&PresetRef> {
        self.preset.as_ref()
    }

    #[must_use]
    pub const fn config(&self) -> &C {
        &self.config
    }

    #[must_use]
    pub fn into_config(self) -> C {
        self.config
    }
}

pub trait RuleDefinition: Debug + Send + Sync {
    type Config: RuleConfig;
    type ValidationError: Error + Send + Sync + 'static;

    fn descriptor(&self) -> &RuleDescriptor;

    fn validate_config(&self, config: &Self::Config) -> Result<(), Self::ValidationError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleMetadataError {
    InvalidRuleSetId,
    InvalidPresetId,
    InvalidSeatCount,
    InvalidDisplayName,
    MissingSeatCount,
}

impl Display for RuleMetadataError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRuleSetId => "rule set ID must contain two lowercase kebab-case segments",
            Self::InvalidPresetId => "preset ID must be lowercase kebab-case",
            Self::InvalidSeatCount => "seat count must be between 2 and 4",
            Self::InvalidDisplayName => "display name must contain 1 to 80 visible characters",
            Self::MissingSeatCount => "at least one supported seat count is required",
        };
        formatter.write_str(message)
    }
}

impl Error for RuleMetadataError {}

fn is_kebab_identifier(value: &str) -> bool {
    if value.is_empty() || value.starts_with('-') || value.ends_with('-') {
        return false;
    }

    let mut previous_was_hyphen = false;
    for byte in value.bytes() {
        let is_hyphen = byte == b'-';
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || is_hyphen)
            || (is_hyphen && previous_was_hyphen)
        {
            return false;
        }
        previous_was_hyphen = is_hyphen;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::{
        EngineVersion, PresetId, PresetRef, RuleDescriptor, RuleSetId, RuleSnapshot, SeatCount,
    };

    #[test]
    fn parses_stable_rule_set_id() {
        let id = RuleSetId::parse("riichi/yonma").expect("valid rule set ID");

        assert_eq!(id.as_str(), "riichi/yonma");
    }

    #[test]
    fn rejects_malformed_rule_set_id() {
        for value in [
            "riichi",
            "riichi/yonma/fast",
            "Riichi/yonma",
            "riichi//yonma",
            "riichi/-yonma",
        ] {
            assert!(
                RuleSetId::parse(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn parses_and_displays_engine_version() {
        let version: EngineVersion = "12.3.40".parse().expect("valid engine version");

        assert_eq!(version.major(), 12);
        assert_eq!(version.minor(), 3);
        assert_eq!(version.patch(), 40);
        assert_eq!(version.to_string(), "12.3.40");
    }

    #[test]
    fn rejects_ambiguous_engine_versions() {
        for value in ["1", "1.2", "1.2.3.4", "01.2.3", "1.2.-3", "65536.0.0"] {
            assert!(
                value.parse::<EngineVersion>().is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn descriptor_sorts_and_deduplicates_seat_counts() {
        let descriptor = RuleDescriptor::new(
            RuleSetId::parse("riichi/flexible").expect("valid rule set ID"),
            EngineVersion::new(0, 1, 0),
            "立直麻将",
            [
                SeatCount::new(4).expect("four seats"),
                SeatCount::new(3).expect("three seats"),
                SeatCount::new(4).expect("four seats"),
            ],
        )
        .expect("valid descriptor");

        let counts: Vec<_> = descriptor
            .supported_seat_counts()
            .iter()
            .copied()
            .map(SeatCount::value)
            .collect();
        assert_eq!(counts, vec![3, 4]);
    }

    #[test]
    fn snapshot_keeps_resolved_config_and_preset_revision() {
        #[derive(Clone, Debug, Eq, PartialEq)]
        struct TestConfig {
            initial_points: u32,
        }

        let preset = PresetRef::new(
            PresetId::parse("m-league").expect("valid preset ID"),
            NonZeroU32::new(1).expect("non-zero revision"),
        );
        let snapshot = RuleSnapshot::new(
            RuleSetId::parse("riichi/yonma").expect("valid rule set ID"),
            EngineVersion::new(0, 1, 0),
            Some(preset),
            TestConfig {
                initial_points: 25_000,
            },
        );

        assert_eq!(
            snapshot.schema_version(),
            super::RULE_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(snapshot.rule_set_id().as_str(), "riichi/yonma");
        assert_eq!(snapshot.config().initial_points, 25_000);
        assert_eq!(snapshot.preset().expect("preset").revision().get(), 1,);
    }
}
