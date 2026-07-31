use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const BIND_ADDRESS_ENV: &str = "MAMAHJONG_BIND_ADDRESS";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DATA_DIR_ENV: &str = "MAMAHJONG_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    bind_address: SocketAddr,
    data_dir: PathBuf,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_address = read_optional_env(BIND_ADDRESS_ENV)?;
        let data_dir = read_optional_env(DATA_DIR_ENV)?;
        Self::from_values(bind_address.as_deref(), data_dir.as_deref())
    }

    pub fn from_bind_address(value: Option<&str>) -> Result<Self, ConfigError> {
        Self::from_values(value, None)
    }

    pub fn from_values(
        bind_address: Option<&str>,
        data_dir: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let value = bind_address.unwrap_or(DEFAULT_BIND_ADDRESS);
        let bind_address = value.parse().map_err(|_| ConfigError::InvalidAddress {
            variable: BIND_ADDRESS_ENV,
            value: value.to_owned(),
        })?;
        let data_dir = PathBuf::from(data_dir.unwrap_or(DEFAULT_DATA_DIR));
        if data_dir.as_os_str().is_empty() {
            return Err(ConfigError::EmptyPath {
                variable: DATA_DIR_ENV,
            });
        }
        Ok(Self {
            bind_address,
            data_dir,
        })
    }

    #[must_use]
    pub const fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
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
    EmptyPath {
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
            Self::EmptyPath { variable } => write!(formatter, "{variable} cannot be empty"),
        }
    }
}

fn read_optional_env(variable: &'static str) -> Result<Option<String>, ConfigError> {
    match std::env::var(variable) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::NonUnicode { variable }),
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
    fn parses_data_directory() {
        let config =
            ServerConfig::from_values(None, Some("/var/lib/mamahjong")).expect("valid config");

        assert_eq!(
            config.data_dir(),
            std::path::Path::new("/var/lib/mamahjong")
        );
    }

    #[test]
    fn rejects_address_without_port() {
        let error =
            ServerConfig::from_bind_address(Some("127.0.0.1")).expect_err("port is required");

        assert!(matches!(error, ConfigError::InvalidAddress { .. }));
    }
}
