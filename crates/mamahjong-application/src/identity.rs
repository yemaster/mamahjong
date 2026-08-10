use std::fmt::{self, Debug, Formatter};

use mahjong_core::{SessionId, UserId};

use crate::{ApplicationError, ErrorCode, MusicScene};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountStatus {
    Active,
    Suspended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRole {
    Player,
    Administrator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TitleSummary {
    id: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterSummary {
    id: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankSummary {
    rule_set_id: String,
    queue_id: String,
    rank: String,
    points: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Nickname(String);

impl Nickname {
    pub fn parse(value: impl Into<String>) -> Result<Self, ApplicationError> {
        let value = value.into();
        let trimmed = value.trim();
        let length = trimmed.chars().count();
        if !(2..=24).contains(&length) || trimmed.chars().any(char::is_control) {
            return Err(ApplicationError::new(
                ErrorCode::InvalidNickname,
                "nickname must contain 2 to 24 non-control characters",
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserProfile {
    nickname: Nickname,
    equipped_title: Option<TitleSummary>,
    selected_character: Option<CharacterSummary>,
    selected_outfit_id: Option<String>,
    avatar_path: Option<String>,
    selected_tablecloth_id: Option<String>,
    selected_lobby_music_id: Option<String>,
    selected_match_music_id: Option<String>,
    selected_riichi_music_id: Option<String>,
    ranks: Vec<RankSummary>,
}

impl UserProfile {
    #[must_use]
    pub fn new(nickname: Nickname) -> Self {
        Self {
            nickname,
            equipped_title: None,
            selected_character: None,
            selected_outfit_id: None,
            avatar_path: None,
            selected_tablecloth_id: None,
            selected_lobby_music_id: None,
            selected_match_music_id: None,
            selected_riichi_music_id: None,
            ranks: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn restore(
        nickname: Nickname,
        selected_character: Option<CharacterSummary>,
        selected_outfit_id: Option<String>,
        avatar_path: Option<String>,
        selected_tablecloth_id: Option<String>,
        selected_lobby_music_id: Option<String>,
        selected_match_music_id: Option<String>,
        selected_riichi_music_id: Option<String>,
    ) -> Self {
        Self {
            nickname,
            equipped_title: None,
            selected_character,
            selected_outfit_id,
            avatar_path,
            selected_tablecloth_id,
            selected_lobby_music_id,
            selected_match_music_id,
            selected_riichi_music_id,
            ranks: Vec::new(),
        }
    }

    #[must_use]
    pub const fn nickname(&self) -> &Nickname {
        &self.nickname
    }

    #[must_use]
    pub const fn equipped_title(&self) -> Option<&TitleSummary> {
        self.equipped_title.as_ref()
    }

    #[must_use]
    pub const fn selected_character(&self) -> Option<&CharacterSummary> {
        self.selected_character.as_ref()
    }

    #[must_use]
    pub fn selected_outfit_id(&self) -> Option<&str> {
        self.selected_outfit_id.as_deref()
    }

    #[must_use]
    pub fn avatar_path(&self) -> Option<&str> {
        self.avatar_path.as_deref()
    }

    #[must_use]
    pub fn selected_tablecloth_id(&self) -> Option<&str> {
        self.selected_tablecloth_id.as_deref()
    }

    #[must_use]
    pub fn selected_lobby_music_id(&self) -> Option<&str> {
        self.selected_lobby_music_id.as_deref()
    }

    #[must_use]
    pub fn selected_match_music_id(&self) -> Option<&str> {
        self.selected_match_music_id.as_deref()
    }

    #[must_use]
    pub fn selected_riichi_music_id(&self) -> Option<&str> {
        self.selected_riichi_music_id.as_deref()
    }

    #[must_use]
    pub fn ranks(&self) -> &[RankSummary] {
        &self.ranks
    }

    pub(crate) fn rename(&mut self, nickname: Nickname) {
        self.nickname = nickname;
    }

    pub(crate) fn select_presentation(
        &mut self,
        character: CharacterSummary,
        outfit_id: String,
        avatar_path: String,
    ) {
        self.selected_character = Some(character);
        self.selected_outfit_id = Some(outfit_id);
        self.avatar_path = Some(avatar_path);
    }

    pub(crate) fn select_tablecloth(&mut self, tablecloth_id: String) {
        self.selected_tablecloth_id = Some(tablecloth_id);
    }

    pub(crate) fn select_music(&mut self, scene: MusicScene, track_id: String) {
        match scene {
            MusicScene::Lobby => self.selected_lobby_music_id = Some(track_id),
            MusicScene::Match => self.selected_match_music_id = Some(track_id),
            MusicScene::Riichi => self.selected_riichi_music_id = Some(track_id),
        }
    }

    pub(crate) fn clear_riichi_music(&mut self) {
        self.selected_riichi_music_id = None;
    }
}

impl TitleSummary {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl CharacterSummary {
    #[must_use]
    pub fn new(id: String, name: String) -> Self {
        Self { id, name }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl RankSummary {
    #[must_use]
    pub fn rule_set_id(&self) -> &str {
        &self.rule_set_id
    }

    #[must_use]
    pub fn queue_id(&self) -> &str {
        &self.queue_id
    }

    #[must_use]
    pub fn rank(&self) -> &str {
        &self.rank
    }

    #[must_use]
    pub const fn points(&self) -> i32 {
        self.points
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User {
    id: UserId,
    version: u64,
    login_name_canonical: String,
    status: AccountStatus,
    role: AccountRole,
    profile: UserProfile,
}

impl User {
    pub(crate) fn new(login_name_canonical: String, nickname: Nickname, role: AccountRole) -> Self {
        Self {
            id: UserId::new(),
            version: 1,
            login_name_canonical,
            status: AccountStatus::Active,
            role,
            profile: UserProfile::new(nickname),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        id: UserId,
        version: u64,
        login_name_canonical: String,
        status: AccountStatus,
        role: AccountRole,
        nickname: Nickname,
        selected_character: Option<CharacterSummary>,
        selected_outfit_id: Option<String>,
        avatar_path: Option<String>,
        selected_tablecloth_id: Option<String>,
        selected_lobby_music_id: Option<String>,
        selected_match_music_id: Option<String>,
        selected_riichi_music_id: Option<String>,
    ) -> Self {
        Self {
            id,
            version,
            login_name_canonical,
            status,
            role,
            profile: UserProfile::restore(
                nickname,
                selected_character,
                selected_outfit_id,
                avatar_path,
                selected_tablecloth_id,
                selected_lobby_music_id,
                selected_match_music_id,
                selected_riichi_music_id,
            ),
        }
    }

    #[must_use]
    pub const fn id(&self) -> &UserId {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub const fn status(&self) -> AccountStatus {
        self.status
    }

    #[must_use]
    pub const fn role(&self) -> AccountRole {
        self.role
    }

    #[must_use]
    pub fn login_name(&self) -> &str {
        &self.login_name_canonical
    }

    #[must_use]
    pub const fn profile(&self) -> &UserProfile {
        &self.profile
    }

    pub(crate) fn profile_mut(&mut self) -> &mut UserProfile {
        &mut self.profile
    }

    pub(crate) fn increment_version(&mut self) {
        self.version += 1;
    }

    pub(crate) fn rename(&mut self, nickname: Nickname) {
        self.profile.rename(nickname);
        self.version += 1;
    }

    pub(crate) fn select_presentation(
        &mut self,
        character: CharacterSummary,
        outfit_id: String,
        avatar_path: String,
    ) {
        self.profile
            .select_presentation(character, outfit_id, avatar_path);
        self.version += 1;
    }

    pub(crate) fn select_tablecloth(&mut self, tablecloth_id: String) {
        self.profile.select_tablecloth(tablecloth_id);
        self.version += 1;
    }

    pub(crate) fn select_music(&mut self, scene: MusicScene, track_id: String) {
        self.profile.select_music(scene, track_id);
        self.version += 1;
    }

    pub(crate) fn set_status(&mut self, status: AccountStatus) {
        if self.status != status {
            self.status = status;
            self.version += 1;
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Session {
    id: SessionId,
    user_id: UserId,
    token: String,
}

impl Session {
    pub(crate) fn new(user_id: UserId, token: String) -> Self {
        Self {
            id: SessionId::new(),
            user_id,
            token,
        }
    }

    pub(crate) fn restore(id: SessionId, user_id: UserId, token: String) -> Self {
        Self { id, user_id, token }
    }

    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl Debug for Session {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("id", &self.id)
            .field("user_id", &self.user_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}
