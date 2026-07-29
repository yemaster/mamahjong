use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;

const BIND_ADDRESS_ENV: &str = "MAMAHJONG_BIND_ADDRESS";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    bind_address: SocketAddr,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let value = match std::env::var(BIND_ADDRESS_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(ConfigError::NonUnicode {
                    variable: BIND_ADDRESS_ENV,
                });
            }
        };
        Self::from_bind_address(value.as_deref())
    }

    pub fn from_bind_address(value: Option<&str>) -> Result<Self, ConfigError> {
        let value = value.unwrap_or(DEFAULT_BIND_ADDRESS);
        let bind_address = value.parse().map_err(|_| ConfigError::InvalidAddress {
            variable: BIND_ADDRESS_ENV,
            value: value.to_owned(),
        })?;
        Ok(Self { bind_address })
    }

    #[must_use]
    pub const fn bind_address(self) -> SocketAddr {
        self.bind_address
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidAddress {
        variable: &'static str,
        value: String,
    },
    NonUnicode {
        variable: &'static str,
    },
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAddress { variable, value } => {
                write!(
                    formatter,
                    "{variable} is not a valid socket address: {value}"
                )
            }
            Self::NonUnicode { variable } => {
                write!(formatter, "{variable} is not valid Unicode")
            }
        }
    }
}

impl Error for ConfigError {}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::{ConfigError, ServerConfig};

    #[test]
    fn uses_loopback_default() {
        let config = ServerConfig::from_bind_address(None).expect("default configuration");

        assert_eq!(
            config.bind_address(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080)
        );
    }

    #[test]
    fn parses_explicit_address() {
        let config =
            ServerConfig::from_bind_address(Some("0.0.0.0:9000")).expect("valid configuration");

        assert_eq!(config.bind_address().port(), 9000);
        assert!(config.bind_address().ip().is_unspecified());
    }

    #[test]
    fn rejects_address_without_port() {
        let error =
            ServerConfig::from_bind_address(Some("127.0.0.1")).expect_err("port is required");

        assert!(matches!(error, ConfigError::InvalidAddress { .. }));
    }
}
