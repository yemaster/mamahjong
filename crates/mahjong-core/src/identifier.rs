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
    };
}

define_entity_id!(UserId, "user_", "user ID");
define_entity_id!(SessionId, "session_", "session ID");
define_entity_id!(RoomId, "room_", "room ID");
define_entity_id!(MatchId, "match_", "match ID");
define_entity_id!(CommandId, "cmd_", "command ID");
define_entity_id!(TicketId, "ticket_", "ticket ID");
define_entity_id!(ConnectionId, "connection_", "connection ID");

#[cfg(test)]
mod tests {
    use super::{CommandId, RoomId};

    const UUID_V7: &str = "018f22e2-7c30-7cc4-98c4-dc0c0c07398f";

    #[test]
    fn accepts_matching_prefixed_uuid_v7() {
        let value = format!("room_{UUID_V7}");
        let id = RoomId::parse(&value).expect("valid room ID");

        assert_eq!(id.as_str(), value);
        assert_eq!(id.to_string(), value);
    }

    #[test]
    fn rejects_another_entity_prefix() {
        let error = CommandId::parse(format!("room_{UUID_V7}")).expect_err("wrong prefix");

        assert_eq!(error.kind(), "command ID");
        assert_eq!(error.reason(), "unexpected prefix");
    }

    #[test]
    fn rejects_non_v7_uuid() {
        let error = RoomId::parse("room_550e8400-e29b-41d4-a716-446655440000")
            .expect_err("UUIDv4 must be rejected");

        assert_eq!(error.reason(), "UUID version must be 7");
    }

    #[test]
    fn rejects_uppercase_uuid() {
        let error = RoomId::parse("room_018F22E2-7C30-7CC4-98C4-DC0C0C07398F")
            .expect_err("uppercase text must be rejected");

        assert_eq!(error.reason(), "expected a lowercase UUIDv7");
    }
}
