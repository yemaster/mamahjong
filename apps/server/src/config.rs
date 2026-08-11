use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const BIND_ADDRESS_ENV: &str = "MAMAHJONG_BIND_ADDRESS";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DATA_DIR_ENV: &str = "MAMAHJONG_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";
const ADMIN_WEB_DIR_ENV: &str = "MAMAHJONG_ADMIN_WEB_DIR";
const DEFAULT_ADMIN_WEB_DIR: &str = "apps/admin-web/dist";
const GAME_WEB_DIR_ENV: &str = "MAMAHJONG_GAME_WEB_DIR";
const DEFAULT_GAME_WEB_DIR: &str = "apps/game-web/dist";
const DATABASE_URL_ENV: &str = "MAMAHJONG_DATABASE_URL";
const ADMIN_LOGIN_ENV: &str = "MAMAHJONG_ADMIN_LOGIN_NAME";
const ADMIN_PASSWORD_ENV: &str = "MAMAHJONG_ADMIN_PASSWORD";
const ADMIN_ALLOW_INSECURE_PASSWORD_ENV: &str = "MAMAHJONG_ADMIN_ALLOW_INSECURE_PASSWORD";
const ADMIN_NICKNAME_ENV: &str = "MAMAHJONG_ADMIN_NICKNAME";
const ADMIN_COOKIE_SECURE_ENV: &str = "MAMAHJONG_ADMIN_COOKIE_SECURE";

#[derive(Clone, Eq, PartialEq)]
pub struct ServerConfig {
    bind_address: SocketAddr,
    data_dir: PathBuf,
    admin_web_dir: PathBuf,
    game_web_dir: PathBuf,
    database_url: Option<String>,
    administrator: Option<AdministratorBootstrap>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct AdministratorBootstrap {
    login_name: String,
    password: String,
    nickname: String,
    cookie_secure: bool,
    allow_insecure_password: bool,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_address = read_optional_env(BIND_ADDRESS_ENV)?;
        let data_dir = read_optional_env(DATA_DIR_ENV)?;
        let admin_web_dir = read_optional_env(ADMIN_WEB_DIR_ENV)?;
        let game_web_dir = read_optional_env(GAME_WEB_DIR_ENV)?;
        let database_url = read_optional_env(DATABASE_URL_ENV)?;
        let allow_insecure_admin_password =
            match read_optional_env(ADMIN_ALLOW_INSECURE_PASSWORD_ENV)?
                .as_deref()
                .unwrap_or("false")
            {
                "true" => true,
                "false" => false,
                value => {
                    return Err(ConfigError::InvalidBoolean {
                        variable: ADMIN_ALLOW_INSECURE_PASSWORD_ENV,
                        value: value.to_owned(),
                    });
                }
            };
        let mut config = Self::from_values_with_web(
            bind_address.as_deref(),
            data_dir.as_deref(),
            admin_web_dir.as_deref(),
            game_web_dir.as_deref(),
        )?;
        config.database_url = database_url.filter(|value| !value.trim().is_empty());
        config.administrator = administrator_from_values(
            read_optional_env(ADMIN_LOGIN_ENV)?.as_deref(),
            read_optional_env(ADMIN_PASSWORD_ENV)?.as_deref(),
            read_optional_env(ADMIN_NICKNAME_ENV)?.as_deref(),
            read_optional_env(ADMIN_COOKIE_SECURE_ENV)?.as_deref(),
            allow_insecure_admin_password,
        )?;
        Ok(config)
    }

    pub fn from_bind_address(value: Option<&str>) -> Result<Self, ConfigError> {
        Self::from_values(value, None)
    }

    pub fn from_values(
        bind_address: Option<&str>,
        data_dir: Option<&str>,
    ) -> Result<Self, ConfigError> {
        Self::from_values_with_web(bind_address, data_dir, None, None)
    }

    fn from_values_with_web(
        bind_address: Option<&str>,
        data_dir: Option<&str>,
        admin_web_dir: Option<&str>,
        game_web_dir: Option<&str>,
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
        let admin_web_dir = PathBuf::from(admin_web_dir.unwrap_or(DEFAULT_ADMIN_WEB_DIR));
        if admin_web_dir.as_os_str().is_empty() {
            return Err(ConfigError::EmptyPath {
                variable: ADMIN_WEB_DIR_ENV,
            });
        }
        let game_web_dir = PathBuf::from(game_web_dir.unwrap_or(DEFAULT_GAME_WEB_DIR));
        if game_web_dir.as_os_str().is_empty() {
            return Err(ConfigError::EmptyPath {
                variable: GAME_WEB_DIR_ENV,
            });
        }
        Ok(Self {
            bind_address,
            data_dir,
            admin_web_dir,
            game_web_dir,
            database_url: None,
            administrator: None,
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

    #[must_use]
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    #[must_use]
    pub fn admin_web_dir(&self) -> &Path {
        &self.admin_web_dir
    }

    #[must_use]
    pub fn game_web_dir(&self) -> &Path {
        &self.game_web_dir
    }

    #[must_use]
    pub fn database_url(&self) -> Option<&str> {
        self.database_url.as_deref()
    }

    #[must_use]
    pub const fn administrator(&self) -> Option<&AdministratorBootstrap> {
        self.administrator.as_ref()
    }
}

impl AdministratorBootstrap {
    #[must_use]
    pub fn login_name(&self) -> &str {
        &self.login_name
    }

    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }

    #[must_use]
    pub const fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }

    #[must_use]
    pub const fn allow_insecure_password(&self) -> bool {
        self.allow_insecure_password
    }
}

impl Debug for ServerConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServerConfig")
            .field("bind_address", &self.bind_address)
            .field("data_dir", &self.data_dir)
            .field("admin_web_dir", &self.admin_web_dir)
            .field("game_web_dir", &self.game_web_dir)
            .field("database_enabled", &self.database_url.is_some())
            .field("administrator_enabled", &self.administrator.is_some())
            .finish()
    }
}

impl Debug for AdministratorBootstrap {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdministratorBootstrap")
            .field("login_name", &self.login_name)
            .field("password", &"[REDACTED]")
            .field("nickname", &self.nickname)
            .field("cookie_secure", &self.cookie_secure)
            .field("allow_insecure_password", &self.allow_insecure_password)
            .finish()
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
    InvalidAdminPassword,
    InvalidBoolean {
        variable: &'static str,
        value: String,
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
            Self::InvalidAdminPassword => {
                formatter.write_str("MAMAHJONG_ADMIN_PASSWORD must contain 12 to 128 bytes")
            }
            Self::InvalidBoolean { variable, value } => {
                write!(formatter, "{variable} must be true or false, got {value}")
            }
        }
    }
}

fn administrator_from_values(
    login_name: Option<&str>,
    password: Option<&str>,
    nickname: Option<&str>,
    cookie_secure: Option<&str>,
    allow_insecure_password: bool,
) -> Result<Option<AdministratorBootstrap>, ConfigError> {
    let Some(password) = password.filter(|password| !password.is_empty()) else {
        return Ok(None);
    };
    let minimum_password_bytes = if allow_insecure_password { 8 } else { 12 };
    if !(minimum_password_bytes..=128).contains(&password.len()) {
        return Err(ConfigError::InvalidAdminPassword);
    }
    let cookie_secure = match cookie_secure.unwrap_or("false") {
        "true" => true,
        "false" => false,
        value => {
            return Err(ConfigError::InvalidBoolean {
                variable: ADMIN_COOKIE_SECURE_ENV,
                value: value.to_owned(),
            });
        }
    };
    Ok(Some(AdministratorBootstrap {
        login_name: login_name.unwrap_or("admin").to_owned(),
        password: password.to_owned(),
        nickname: nickname.unwrap_or("管理员").to_owned(),
        cookie_secure,
        allow_insecure_password,
    }))
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

    use super::{ConfigError, ServerConfig, administrator_from_values};

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

    #[test]
    fn administrator_is_disabled_without_password() {
        assert!(
            administrator_from_values(Some("admin"), None, Some("管理员"), None, false)
                .expect("configuration")
                .is_none()
        );
    }

    #[test]
    fn administrator_password_and_secure_cookie_are_validated() {
        let administrator = administrator_from_values(
            Some("operator"),
            Some("long-admin-password"),
            Some("运营人员"),
            Some("true"),
            false,
        )
        .expect("configuration")
        .expect("administrator");
        assert_eq!(administrator.login_name(), "operator");
        assert!(administrator.cookie_secure());
        assert!(matches!(
            administrator_from_values(None, Some("short"), None, None, false),
            Err(ConfigError::InvalidAdminPassword)
        ));
        assert!(administrator_from_values(None, Some("abc123456"), None, None, true).is_ok());
    }
}
