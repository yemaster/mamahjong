use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AggregateVersion(u64);

impl AggregateVersion {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Result<Self, VersionOverflow> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(VersionOverflow),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionOverflow;

impl Display for VersionOverflow {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("aggregate version overflow")
    }
}

impl Error for VersionOverflow {}

#[cfg(test)]
mod tests {
    use super::AggregateVersion;

    #[test]
    fn advances_monotonically() {
        let next = AggregateVersion::INITIAL
            .checked_next()
            .expect("initial version can advance");

        assert_eq!(next.value(), 1);
    }

    #[test]
    fn detects_overflow() {
        let result = AggregateVersion::new(u64::MAX).checked_next();

        assert!(result.is_err());
    }
}
