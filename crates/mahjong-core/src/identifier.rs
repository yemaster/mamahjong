use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

const UUID_V7_TEXT_LENGTH: usize = 36;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdParseError {
    kind: &'static str,
    reason: &'static str,
}

impl IdParseError {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl Display for IdParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.kind, self.reason)
    }
}

impl Error for IdParseError {}

fn validate_prefixed_uuid_v7(
    value: &str,
    prefix: &'static str,
    kind: &'static str,
) -> Result<(), IdParseError> {
    let Some(uuid) = value.strip_prefix(prefix) else {
        return Err(IdParseError {
            kind,
            reason: "unexpected prefix",
        });
    };

    if uuid.len() != UUID_V7_TEXT_LENGTH {
        return Err(IdParseError {
            kind,
            reason: "expected a UUIDv7",
        });
    }

    for (index, byte) in uuid.bytes().enumerate() {
        let valid = match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
        };
        if !valid {
            return Err(IdParseError {
                kind,
                reason: "expected a lowercase UUIDv7",
            });
        }
    }

    if uuid.as_bytes()[14] != b'7' {
        return Err(IdParseError {
            kind,
            reason: "UUID version must be 7",
        });
    }

    if !matches!(uuid.as_bytes()[19], b'8' | b'9' | b'a' | b'b') {
        return Err(IdParseError {
            kind,
            reason: "invalid UUID variant",
        });
    }

    Ok(())
}

macro_rules! define_entity_id {
    ($name:ident, $prefix:literal, $kind:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(format!(concat!($prefix, "{}"), uuid::Uuid::now_v7()))
            }

            pub fn parse(value: impl Into<String>) -> Result<Self, IdParseError> {
                let value = value.into();
                validate_prefixed_uuid_v7(&value, $prefix, $kind)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

define_entity_id!(UserId, "user_", "user ID");
define_entity_id!(SessionId, "session_", "session ID");
define_entity_id!(MatchId, "match_", "match ID");
define_entity_id!(CommandId, "cmd_", "command ID");
define_entity_id!(TicketId, "ticket_", "ticket ID");
define_entity_id!(ConnectionId, "connection_", "connection ID");

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RoomId(String);

impl RoomId {
    #[must_use]
    pub fn new() -> Self {
        let bytes = uuid::Uuid::now_v7().into_bytes();
        let random = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        Self(format!("{:06}", random % 1_000_000))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, IdParseError> {
        let value = value.into();
        if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(IdParseError {
                kind: "room ID",
                reason: "expected exactly six digits",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RoomId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RoomId {
    type Err = IdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Default for RoomId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandId, RoomId, UserId};

    const UUID_V7: &str = "018f22e2-7c30-7cc4-98c4-dc0c0c07398f";

    #[test]
    fn accepts_six_digit_room_codes() {
        let value = "042861";
        let id = RoomId::parse(value).expect("valid room ID");

        assert_eq!(id.as_str(), value);
        assert_eq!(id.to_string(), value);
    }

    #[test]
    fn generates_unique_parseable_uuid_v7_ids() {
        let first = UserId::new();
        let second = UserId::new();

        assert_ne!(first, second);
        assert_eq!(UserId::parse(first.as_str()), Ok(first));
    }

    #[test]
    fn rejects_another_entity_prefix() {
        let error = CommandId::parse(format!("room_{UUID_V7}")).expect_err("wrong prefix");

        assert_eq!(error.kind(), "command ID");
        assert_eq!(error.reason(), "unexpected prefix");
    }

    #[test]
    fn generates_six_digit_room_codes() {
        let room_id = RoomId::new();
        assert_eq!(room_id.as_str().len(), 6);
        assert!(room_id.as_str().bytes().all(|byte| byte.is_ascii_digit()));
    }

    #[test]
    fn rejects_non_numeric_room_codes() {
        let error = RoomId::parse("12A456").expect_err("letters must be rejected");
        assert_eq!(error.reason(), "expected exactly six digits");
    }
}
