use serde::{Deserialize, Serialize};

use crate::{ApplicationError, ErrorCode};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Tablecloth {
    id: String,
    version: u64,
    name: String,
    texture_path: String,
    enabled: bool,
    is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SaveTablecloth {
    pub id: String,
    pub name: String,
    pub texture_path: String,
    pub enabled: bool,
    pub is_default: bool,
}

impl Tablecloth {
    pub(crate) fn from_command(
        command: SaveTablecloth,
        version: u64,
    ) -> Result<Self, ApplicationError> {
        validate_id(&command.id)?;
        validate_name(&command.name)?;
        validate_path(&command.texture_path)?;
        if command.is_default && !command.enabled {
            return Err(invalid_tablecloth(
                "the default tablecloth must remain enabled",
            ));
        }
        Ok(Self {
            id: command.id,
            version,
            name: command.name.trim().to_owned(),
            texture_path: command.texture_path,
            enabled: command.enabled,
            is_default: command.is_default,
        })
    }

    pub(crate) fn restore(command: SaveTablecloth, version: u64) -> Result<Self, ApplicationError> {
        Self::from_command(command, version)
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn texture_path(&self) -> &str {
        &self.texture_path
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.is_default
    }

    pub(crate) fn clear_default(&mut self) {
        self.is_default = false;
    }
}

#[must_use]
pub fn built_in_tablecloths() -> Vec<SaveTablecloth> {
    let tablecloth = |id: &str, name: &str, is_default: bool| SaveTablecloth {
        id: id.to_owned(),
        name: name.to_owned(),
        texture_path: format!("/game/assets/local-game-assets/mahjong-soul/tablecloths/{id}.png"),
        enabled: true,
        is_default,
    };
    vec![
        tablecloth("peacock-green", "孔雀绿", true),
        tablecloth("lotus-purple", "莲藕紫", false),
        tablecloth("flowers-under-moon", "花月夜", false),
        tablecloth("coal-gray", "炭灰", false),
        tablecloth("official-tournament", "官方赛事", false),
    ]
}

fn validate_id(value: &str) -> Result<(), ApplicationError> {
    let valid = (2..=64).contains(&value.len())
        && value.is_ascii()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character));
    if valid {
        Ok(())
    } else {
        Err(invalid_tablecloth(
            "tablecloth IDs must use 2 to 64 ASCII letters, digits, hyphens, or underscores",
        ))
    }
}

fn validate_name(value: &str) -> Result<(), ApplicationError> {
    let value = value.trim();
    if (1..=64).contains(&value.chars().count()) && !value.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(invalid_tablecloth(
            "tablecloth name must contain 1 to 64 non-control characters",
        ))
    }
}

fn validate_path(value: &str) -> Result<(), ApplicationError> {
    if (value.starts_with("/game/assets/") || value.starts_with("/user-assets/"))
        && !value.chars().any(char::is_control)
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        && value.len() <= 512
    {
        Ok(())
    } else {
        Err(invalid_tablecloth(
            "tablecloth texture path must use /game/assets/ or /user-assets/",
        ))
    }
}

fn invalid_tablecloth(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidTablecloth, message)
}
