use serde::{Deserialize, Serialize};

use crate::{ApplicationError, ErrorCode};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterAsset {
    pub name: String,
    pub path: String,
}

/// 一条语音对应牌桌上的哪个动作。
///
/// 客户端照它挑该放哪一条，不看 `name`——名字是管理端可以随手改的展示文案，
/// 改了不该把声音弄丢。没标 `kind` 的语音（大厅问候之类）只在试听里露面。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKind {
    Riichi,
    DoubleRiichi,
    Chi,
    Pon,
    Kan,
    Nuki,
    Ron,
    Tsumo,
}

impl VoiceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Riichi => "riichi",
            Self::DoubleRiichi => "double_riichi",
            Self::Chi => "chi",
            Self::Pon => "pon",
            Self::Kan => "kan",
            Self::Nuki => "nuki",
            Self::Ron => "ron",
            Self::Tsumo => "tsumo",
        }
    }
}

/// 一条角色语音。
///
/// `kind` 是后加的，库里存量的行只有 `name` 和 `path`，缺这个字段要能照常读出来。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterVoice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<VoiceKind>,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterOutfit {
    pub id: String,
    pub name: String,
    pub illustration_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Character {
    id: String,
    version: u64,
    name: String,
    illustration_path: String,
    emotes: Vec<CharacterAsset>,
    voices: Vec<CharacterVoice>,
    outfits: Vec<CharacterOutfit>,
    enabled: bool,
    is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SaveCharacter {
    pub id: String,
    pub name: String,
    pub illustration_path: String,
    pub emotes: Vec<CharacterAsset>,
    pub voices: Vec<CharacterVoice>,
    pub outfits: Vec<CharacterOutfit>,
    pub enabled: bool,
    pub is_default: bool,
}

impl Character {
    pub(crate) fn from_command(
        command: SaveCharacter,
        version: u64,
    ) -> Result<Self, ApplicationError> {
        validate_id(&command.id)?;
        validate_label(&command.name, "character name")?;
        validate_path(&command.illustration_path)?;
        for asset in &command.emotes {
            validate_label(&asset.name, "asset name")?;
            validate_path(&asset.path)?;
        }
        for voice in &command.voices {
            validate_label(&voice.name, "asset name")?;
            validate_path(&voice.path)?;
        }
        for outfit in &command.outfits {
            validate_id(&outfit.id)?;
            validate_label(&outfit.name, "outfit name")?;
            validate_path(&outfit.illustration_path)?;
        }
        if !command
            .outfits
            .iter()
            .any(|outfit| outfit.illustration_path == command.illustration_path)
        {
            return Err(invalid_character(
                "the main illustration must belong to one outfit",
            ));
        }
        if command.is_default && !command.enabled {
            return Err(invalid_character(
                "the default character must remain enabled",
            ));
        }
        Ok(Self {
            id: command.id,
            version,
            name: command.name.trim().to_owned(),
            illustration_path: command.illustration_path,
            emotes: command.emotes,
            voices: command.voices,
            outfits: command.outfits,
            enabled: command.enabled,
            is_default: command.is_default,
        })
    }

    pub(crate) fn restore(command: SaveCharacter, version: u64) -> Result<Self, ApplicationError> {
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
    pub fn illustration_path(&self) -> &str {
        &self.illustration_path
    }

    #[must_use]
    pub fn emotes(&self) -> &[CharacterAsset] {
        &self.emotes
    }

    #[must_use]
    pub fn voices(&self) -> &[CharacterVoice] {
        &self.voices
    }

    /// 给存量角色补上操作语音。只在原本一条都没有时用，不覆盖管理端配过的。
    pub(crate) fn fill_voices(&mut self, voices: Vec<CharacterVoice>) {
        self.voices = voices;
    }

    /// 给缺 `kind` 的旧语音按文件名补上动作，返回是否真的改了东西。
    ///
    /// 只填空，认不出来的（大厅问候之类）保持没有 kind，已经有 kind 的一概不动
    /// ——那可能是后台手工指定的，不该被路径覆盖掉。
    pub(crate) fn stamp_voice_kinds(&mut self) -> bool {
        let mut changed = false;
        for voice in &mut self.voices {
            if voice.kind.is_some() {
                continue;
            }
            if let Some(kind) = voice_kind_for_path(&voice.path) {
                voice.kind = Some(kind);
                changed = true;
            }
        }
        changed
    }

    #[must_use]
    pub fn outfits(&self) -> &[CharacterOutfit] {
        &self.outfits
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
pub fn ichihime_default() -> SaveCharacter {
    let root = "/game/assets/local-characters/mahjong-soul/ichihime";
    let outfit = |id: &str, name: &str, file: &str| CharacterOutfit {
        id: id.to_owned(),
        name: name.to_owned(),
        illustration_path: format!("{root}/outfits/{file}.png"),
    };
    let asset = |name: &str, directory: &str, file: &str, extension: &str| CharacterAsset {
        name: name.to_owned(),
        path: format!("{root}/{directory}/{file}.{extension}"),
    };
    SaveCharacter {
        id: "ichihime".to_owned(),
        name: "一姬".to_owned(),
        illustration_path: format!("{root}/outfits/yiji.png"),
        emotes: [
            ("开心", "7"),
            ("疑惑", "3"),
            ("得意", "5"),
            ("惊讶", "l_1"),
            ("生气", "1"),
            ("庆祝", "l_2"),
            ("委屈", "997"),
            ("期待", "10"),
            ("微笑", "8"),
            ("害羞", "996"),
            ("困倦", "983"),
            ("欢呼", "888"),
            ("平静", "0"),
            ("无奈", "2"),
            ("雀跃", "l_3"),
            ("认真", "l_4"),
            ("不满", "4"),
            ("发呆", "984"),
            ("思考", "12"),
            ("紧张", "6"),
            ("胜利", "11"),
            ("加油", "966"),
        ]
        .into_iter()
        .map(|(name, file)| asset(name, "emotes", file, "png"))
        .collect(),
        voices: action_voices("ichihime")
            .into_iter()
            .chain(
                [
                    ("大厅问候一", "lobby_normal1"),
                    ("大厅问候二", "lobby_normal2"),
                    ("大厅问候三", "lobby_normal3"),
                    ("大厅问候四", "lobby_normal4"),
                    ("大厅问候五", "lobby_normal5"),
                    ("登录", "lobby_playerlogin"),
                    ("赠礼", "lobby_gift"),
                    ("新年", "lobby_newyear"),
                    ("好感提升", "lobby_levelup1"),
                    ("好感满级", "lobby_levelmax"),
                ]
                .into_iter()
                .map(|(name, file)| CharacterVoice {
                    kind: None,
                    name: name.to_owned(),
                    path: voice_path("ichihime", file),
                }),
            )
            .collect(),
        outfits: vec![
            outfit("default", "初始装扮", "yiji"),
            outfit("contract", "契约", "yiji_0"),
            outfit("beach", "海滩派对", "yiji_haitanpaidui"),
            outfit("new-year", "新春参拜", "yiji_xinnianchuzhi"),
            outfit("warrior", "一姬当千", "yiji_SP"),
            outfit("spring-song", "绮春歌", "yiji_CJ"),
        ],
        enabled: true,
        is_default: true,
    }
}

#[must_use]
pub fn yuan_xiao_default() -> SaveCharacter {
    let root = "/game/assets/local-characters/mahjong-soul/yuan-xiao";
    let outfit = |id: &str, name: &str, file: &str| CharacterOutfit {
        id: id.to_owned(),
        name: name.to_owned(),
        illustration_path: format!("{root}/outfits/{file}.png"),
    };
    let asset = |name: &str, file: &str| CharacterAsset {
        name: name.to_owned(),
        path: format!("{root}/emotes/{file}.png"),
    };
    SaveCharacter {
        id: "yuan-xiao".to_owned(),
        name: "元宵".to_owned(),
        illustration_path: format!("{root}/outfits/default.png"),
        emotes: [
            ("开心", "0"),
            ("不满", "1"),
            ("无奈", "2"),
            ("疑惑", "3"),
            ("微笑", "4"),
            ("得意", "5"),
            ("紧张", "6"),
            ("期待", "7"),
            ("困倦", "8"),
            ("害羞", "10"),
            ("胜利", "11"),
            ("思考", "12"),
            ("惊讶", "14"),
            ("欢呼", "888"),
            ("认真", "935"),
        ]
        .into_iter()
        .map(|(name, file)| asset(name, file))
        .collect(),
        voices: action_voices("yuan-xiao")
            .into_iter()
            .chain(
                [
                    ("大厅问候一", "lobby_normal1"),
                    ("大厅问候二", "lobby_normal2"),
                    ("大厅问候三", "lobby_normal3"),
                    ("大厅问候四", "lobby_normal4"),
                    ("大厅问候五", "lobby_normal5"),
                    ("登录", "lobby_playerlogin"),
                    ("赠礼", "lobby_gift"),
                    ("好感提升", "lobby_levelup1"),
                    ("好感满级", "lobby_levelmax"),
                ]
                .into_iter()
                .map(|(name, file)| CharacterVoice {
                    kind: None,
                    name: name.to_owned(),
                    path: voice_path("yuan-xiao", file),
                }),
            )
            .collect(),
        outfits: vec![
            outfit("default", "初始装扮", "default"),
            outfit("contract", "契约", "contract"),
            outfit("skin", "云踪侠影", "skin"),
        ],
        enabled: true,
        is_default: false,
    }
}

/// 本机演示用的角色。它们的立绘和表情已经在库里，只有操作语音是后补的。
const BUILT_IN_CHARACTER_IDS: [&str; 5] =
    ["ichihime", "kujo-riu", "yagi-yui", "fukuhime", "yuan-xiao"];

fn voice_path(character_id: &str, file: &str) -> String {
    format!("/game/assets/local-characters/mahjong-soul/{character_id}/voices/{file}.mp3")
}

/// 牌桌上会喊的八条语音：动作、默认名字、文件名。四个角色共用同一套文件名。
const ACTION_VOICES: [(VoiceKind, &str, &str); 8] = [
    (VoiceKind::Riichi, "立直", "act_rich"),
    (VoiceKind::DoubleRiichi, "两立直", "act_drich"),
    (VoiceKind::Chi, "吃", "act_chi"),
    (VoiceKind::Pon, "碰", "act_pon"),
    (VoiceKind::Kan, "杠", "act_kan"),
    (VoiceKind::Nuki, "拔北", "act_babei"),
    (VoiceKind::Ron, "荣和", "act_ron"),
    (VoiceKind::Tsumo, "自摸", "act_tumo"),
];

/// 一个角色在牌桌上会喊的八条语音。
#[must_use]
pub fn action_voices(character_id: &str) -> Vec<CharacterVoice> {
    ACTION_VOICES
        .into_iter()
        .map(|(kind, name, file)| CharacterVoice {
            kind: Some(kind),
            name: name.to_owned(),
            path: voice_path(character_id, file),
        })
        .collect()
}

/// 从语音文件路径反推它是哪个动作。
///
/// 早于 `kind` 字段入库的行只有名字和路径，而名字是后台可以随便改的，靠名字
/// 认动作会被一次重命名弄哑。路径里的文件名才是稳定的那一半，所以按它来认。
#[must_use]
fn voice_kind_for_path(path: &str) -> Option<VoiceKind> {
    let file = path.rsplit('/').next()?;
    let stem = file.strip_suffix(".mp3").unwrap_or(file);
    ACTION_VOICES
        .into_iter()
        .find(|(_, _, name)| *name == stem)
        .map(|(kind, _, _)| kind)
}

/// 内建角色该有的操作语音；不是内建角色就没有意见。
#[must_use]
pub fn built_in_action_voices(character_id: &str) -> Option<Vec<CharacterVoice>> {
    BUILT_IN_CHARACTER_IDS
        .contains(&character_id)
        .then(|| action_voices(character_id))
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
        Err(invalid_character(
            "character and outfit IDs must use 2 to 64 ASCII letters, digits, hyphens, or underscores",
        ))
    }
}

fn validate_label(value: &str, field: &str) -> Result<(), ApplicationError> {
    let value = value.trim();
    if (1..=64).contains(&value.chars().count()) && !value.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(invalid_character(format!(
            "{field} must contain 1 to 64 non-control characters"
        )))
    }
}

fn validate_path(value: &str) -> Result<(), ApplicationError> {
    if value.starts_with('/') && !value.chars().any(char::is_control) && value.len() <= 512 {
        Ok(())
    } else {
        Err(invalid_character(
            "character asset paths must be absolute local paths",
        ))
    }
}

fn invalid_character(message: impl Into<String>) -> ApplicationError {
    ApplicationError::new(ErrorCode::InvalidCharacter, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(voices: Vec<CharacterVoice>) -> Character {
        let mut command = ichihime_default();
        command.voices = voices;
        Character::from_command(command, 1).expect("默认角色应当合法")
    }

    fn legacy(name: &str, file: &str) -> CharacterVoice {
        CharacterVoice {
            kind: None,
            name: name.to_owned(),
            path: voice_path("ichihime", file),
        }
    }

    #[test]
    fn legacy_voices_get_their_kind_from_the_file_name() {
        let mut subject = character(vec![legacy("碰", "act_pon"), legacy("自摸", "act_tumo")]);
        assert!(subject.stamp_voice_kinds());
        let kinds: Vec<_> = subject.voices().iter().map(|voice| voice.kind).collect();
        assert_eq!(kinds, vec![Some(VoiceKind::Pon), Some(VoiceKind::Tsumo)]);
    }

    #[test]
    fn a_renamed_voice_is_still_recognised_by_path() {
        /* 后台把名字改成别的，仍然要按路径认出这是碰。 */
        let mut subject = character(vec![legacy("碰！！", "act_pon")]);
        assert!(subject.stamp_voice_kinds());
        assert_eq!(subject.voices()[0].kind, Some(VoiceKind::Pon));
    }

    #[test]
    fn lobby_voices_are_left_without_a_kind() {
        let mut subject = character(vec![legacy("大厅问候一", "lobby_normal1")]);
        assert!(!subject.stamp_voice_kinds());
        assert_eq!(subject.voices()[0].kind, None);
    }

    #[test]
    fn an_existing_kind_is_never_overwritten() {
        /* 管理端手工指定过 kind，就算和文件名对不上也不动它。 */
        let mut subject = character(vec![CharacterVoice {
            kind: Some(VoiceKind::Ron),
            name: "碰".to_owned(),
            path: voice_path("ichihime", "act_pon"),
        }]);
        assert!(!subject.stamp_voice_kinds());
        assert_eq!(subject.voices()[0].kind, Some(VoiceKind::Ron));
    }

    #[test]
    fn every_action_voice_maps_back_to_its_own_kind() {
        for (kind, _, file) in ACTION_VOICES {
            assert_eq!(
                voice_kind_for_path(&voice_path("kujo-riu", file)),
                Some(kind)
            );
        }
    }
}
