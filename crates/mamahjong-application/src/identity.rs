use std::fmt::{self, Debug, Formatter};

use mahjong_core::{SessionId, UserId};

use crate::{ApplicationError, ErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountStatus {
    Active,
    Suspended,
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
    ranks: Vec<RankSummary>,
}

impl UserProfile {
    #[must_use]
    pub fn new(nickname: Nickname) -> Self {
        Self {
            nickname,
            equipped_title: None,
            selected_character: None,
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
    pub fn ranks(&self) -> &[RankSummary] {
        &self.ranks
    }

    pub(crate) fn rename(&mut self, nickname: Nickname) {
        self.nickname = nickname;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User {
    id: UserId,
    version: u64,
    login_name_canonical: String,
    status: AccountStatus,
    profile: UserProfile,
}

impl User {
    pub(crate) fn new(login_name_canonical: String, nickname: Nickname) -> Self {
        Self {
            id: UserId::new(),
            version: 1,
            login_name_canonical,
            status: AccountStatus::Active,
            profile: UserProfile::new(nickname),
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
    pub fn login_name(&self) -> &str {
        &self.login_name_canonical
    }

    #[must_use]
    pub const fn profile(&self) -> &UserProfile {
        &self.profile
    }

    pub(crate) fn rename(&mut self, nickname: Nickname) {
        self.profile.rename(nickname);
        self.version += 1;
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
