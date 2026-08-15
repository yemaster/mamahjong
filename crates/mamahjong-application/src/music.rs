use serde::{Deserialize, Serialize};

use crate::{ApplicationError, ErrorCode};

/// 一首曲子用在哪儿。大厅和对局各自选各自的，互不影响。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicScene {
    Lobby,
    Match,
    Riichi,
}

impl MusicScene {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lobby => "lobby",
            Self::Match => "match",
            Self::Riichi => "riichi",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApplicationError> {
        match value {
            "lobby" => Ok(Self::Lobby),
            "match" => Ok(Self::Match),
            "riichi" => Ok(Self::Riichi),
            _ => Err(invalid_music_track("unknown music scene")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MusicTrack {
    id: String,
    version: u64,
    name: String,
    scene: MusicScene,
    audio_path: String,
    duration_ms: u64,
    enabled: bool,
    is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SaveMusicTrack {
    pub id: String,
    pub name: String,
    pub scene: MusicScene,
    pub audio_path: String,
    pub duration_ms: u64,
    pub enabled: bool,
    pub is_default: bool,
}

impl MusicTrack {
    pub(crate) fn from_command(
        command: SaveMusicTrack,
        version: u64,
    ) -> Result<Self, ApplicationError> {
        validate_id(&command.id)?;
        validate_name(&command.name)?;
        validate_path(&command.audio_path)?;
        if command.is_default && !command.enabled {
            return Err(invalid_music_track("the default track must remain enabled"));
        }
        Ok(Self {
            id: command.id,
            version,
            name: command.name.trim().to_owned(),
            scene: command.scene,
            audio_path: command.audio_path,
            duration_ms: command.duration_ms,
            enabled: command.enabled,
            is_default: command.is_default,
        })
    }

    pub(crate) fn restore(command: SaveMusicTrack, version: u64) -> Result<Self, ApplicationError> {
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
    pub const fn scene(&self) -> MusicScene {
        self.scene
    }

    #[must_use]
    pub fn audio_path(&self) -> &str {
        &self.audio_path
    }

    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
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

/// 本地开发使用的曲目目录。这里只保存元数据，音频文件由部署者自行提供。
#[must_use]
pub fn built_in_music_tracks() -> Vec<SaveMusicTrack> {
    [
        (
            "lobby-default",
            "默认",
            MusicScene::Lobby,
            "lobby-default.mp3",
            75_572,
            true,
        ),
        (
            "fusheng-touxian",
            "浮生偷闲",
            MusicScene::Lobby,
            "fusheng-touxian.mp3",
            198_740,
            false,
        ),
        (
            "zhiying-zhuiguang",
            "织影缀光",
            MusicScene::Lobby,
            "zhiying-zhuiguang.mp3",
            130_795,
            false,
        ),
        (
            "zhuqu-zhiyu",
            "竹取之语",
            MusicScene::Match,
            "zhuqu-zhiyu.mp3",
            71_889,
            true,
        ),
        (
            "chuzhen",
            "出阵",
            MusicScene::Riichi,
            "chuzhen.mp3",
            69_721,
            false,
        ),
        (
            "guangzhouta",
            "广州塔",
            MusicScene::Riichi,
            "guangzhouta.mp3",
            116_532,
            false,
        ),
    ]
    .into_iter()
    .map(
        |(id, name, scene, file_name, duration_ms, is_default)| SaveMusicTrack {
            id: id.to_owned(),
            name: name.to_owned(),
            scene,
            audio_path: format!("/game/assets/local-game-assets/music/{file_name}"),
            duration_ms,
            enabled: true,
            is_default,
        },
    )
    .collect()
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
        Err(invalid_music_track(
            "music IDs must use 2 to 64 ASCII letters, digits, hyphens, or underscores",
        ))
    }
}

fn validate_name(value: &str) -> Result<(), ApplicationError> {
    let value = value.trim();
    if (1..=64).contains(&value.chars().count()) && !value.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(invalid_music_track(
            "music name must contain 1 to 64 non-control characters",
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
        Err(invalid_music_track(
            "music audio path must use /game/assets/ or /user-assets/",
        ))
    }
}

fn invalid_music_track(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidMusicTrack, message)
}
