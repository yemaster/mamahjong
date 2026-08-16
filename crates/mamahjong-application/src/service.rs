use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use mahjong_core::{MatchId, RoomId, TicketId, UserId};
use mahjong_impact::ImpactRoomRuleRequest;
use mahjong_riichi::{RiichiRuleSnapshot, RiichiRules, RiichiVariant, RoomRuleRequest};

use crate::identity_store::PostgresIdentityStore;
use crate::runtime::{GameRuntime, MatchProjection, not_riichi};
use crate::store::MemoryStore;
use crate::{
    AccountRole, AccountStatus, ApplicationError, Character, ClockExpiry, ErrorCode,
    GameRuleSnapshot, MatchEventPage, MatchmakingStatus, MatchmakingTicket, MusicScene, MusicTrack,
    Nickname, ObserverMatch, Room, RoomVisibility, SaveCharacter, SaveMusicTrack, SaveTablecloth,
    Session, SubmitGameCommand, Tablecloth, User, built_in_action_voices, built_in_music_tracks,
    built_in_tablecloths, ichihime_default,
};

const MAX_PASSWORD_BYTES: usize = 128;
static DUMMY_PASSWORD_HASH: OnceLock<Result<String, ApplicationError>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterUser {
    pub login_name: String,
    pub password: String,
    pub nickname: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProfile {
    pub nickname: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatePresentation {
    pub character_id: String,
    pub outfit_id: String,
    pub avatar_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateTablecloth {
    pub tablecloth_id: String,
}

/// 只带上要改的那一项，另一项留 `None` 就保持原样。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateMusic {
    pub lobby_music_id: Option<String>,
    pub match_music_id: Option<String>,
    pub riichi_music_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoomRuleSelection {
    Riichi {
        variant: RiichiVariant,
        request: RoomRuleRequest,
    },
    /// 冲击麻将固定四人，没有变体可选。
    Impact { request: ImpactRoomRuleRequest },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRoom {
    pub name: String,
    pub visibility: RoomVisibility,
    pub rules: RoomRuleSelection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateRoom {
    pub expected_version: u64,
    pub name: Option<String>,
    pub visibility: Option<RoomVisibility>,
    pub rules: Option<RoomRuleSelection>,
}

#[derive(Clone)]
pub struct Application {
    store: Arc<RwLock<MemoryStore>>,
    identity_store: Option<Arc<PostgresIdentityStore>>,
}

impl Application {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn persistence_enabled(&self) -> bool {
        self.identity_store.is_some()
    }

    pub fn connect_postgres(database_url: &str) -> Result<Self, ApplicationError> {
        let identity_store = Arc::new(PostgresIdentityStore::connect(database_url)?);
        let mut store = MemoryStore::default();
        identity_store.load_into(&mut store)?;
        if store.characters.is_empty() {
            let character = Character::from_command(ichihime_default(), 1)?;
            identity_store.upsert_character(&character)?;
            store
                .characters
                .insert(character.id().to_owned(), character);
        }
        /* 操作语音是后加的，库里已有的角色分两种情况：一条语音都没有的内建角色
        在这里补齐；早先存过语音、但那时还没有 kind 字段的（比如一姬）按文件名
        把 kind 补上。两种都只填空，管理端自己配过的一概不碰。 */
        for character in store.characters.values_mut() {
            let changed = if character.voices().is_empty() {
                match built_in_action_voices(character.id()) {
                    Some(voices) => {
                        character.fill_voices(voices);
                        true
                    }
                    None => false,
                }
            } else {
                character.stamp_voice_kinds()
            };
            if changed {
                identity_store.upsert_character(character)?;
            }
        }
        if store.tablecloths.is_empty() {
            for command in built_in_tablecloths() {
                let tablecloth = Tablecloth::from_command(command, 1)?;
                identity_store.upsert_tablecloth(&tablecloth)?;
                store
                    .tablecloths
                    .insert(tablecloth.id().to_owned(), tablecloth);
            }
        }
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            identity_store: Some(identity_store),
        })
    }

    pub fn register(&self, command: RegisterUser) -> Result<(User, Session), ApplicationError> {
        let (user, session) = self.register_with_role(command, AccountRole::Player, true, 10)?;
        Ok((user, session.ok_or_else(internal_error)?))
    }

    pub fn bootstrap_administrator(&self, command: RegisterUser) -> Result<User, ApplicationError> {
        self.bootstrap_administrator_with_minimum(command, 10)
    }

    pub fn bootstrap_development_administrator(
        &self,
        command: RegisterUser,
    ) -> Result<User, ApplicationError> {
        self.bootstrap_administrator_with_minimum(command, 8)
    }

    fn bootstrap_administrator_with_minimum(
        &self,
        command: RegisterUser,
        minimum_password_bytes: usize,
    ) -> Result<User, ApplicationError> {
        let canonical = canonical_login_name(&command.login_name)?;
        let existing = {
            let store = self.read_store()?;
            store
                .login_index
                .get(&canonical)
                .and_then(|user_id| store.users.get(user_id))
                .cloned()
        };
        if let Some(existing) = existing {
            if existing.role() == AccountRole::Administrator {
                validate_password_with_minimum(&command.password, minimum_password_bytes)?;
                let password_hash = hash_password(&command.password)?;
                if let Some(identity_store) = &self.identity_store {
                    identity_store.update_password(existing.id(), &password_hash)?;
                }
                let mut store = self.write_store()?;
                store
                    .password_hashes
                    .insert(existing.id().clone(), password_hash);
                store
                    .sessions
                    .retain(|_, session| session.user_id() != existing.id());
                return Ok(existing);
            }
            return Err(ApplicationError::new(
                ErrorCode::LoginNameTaken,
                "login name is already registered",
            ));
        }
        let (user, _) = self.register_with_role(
            command,
            AccountRole::Administrator,
            false,
            minimum_password_bytes,
        )?;
        Ok(user)
    }

    fn register_with_role(
        &self,
        command: RegisterUser,
        role: AccountRole,
        create_session: bool,
        minimum_password_bytes: usize,
    ) -> Result<(User, Option<Session>), ApplicationError> {
        let login_name = canonical_login_name(&command.login_name)?;
        validate_password_with_minimum(&command.password, minimum_password_bytes)?;
        let nickname = Nickname::parse(command.nickname)?;
        let password_hash = hash_password(&command.password)?;
        let mut store = self.write_store()?;
        if store.login_index.contains_key(&login_name) {
            return Err(ApplicationError::new(
                ErrorCode::LoginNameTaken,
                "login name is already registered",
            ));
        }
        let user = User::new(login_name.clone(), nickname, role);
        let session = create_session
            .then(|| new_session(user.id().clone()))
            .transpose()?;
        if let Some(identity_store) = &self.identity_store {
            identity_store.insert_user(&user, &password_hash, session.as_ref())?;
        }
        store.login_index.insert(login_name, user.id().clone());
        store
            .password_hashes
            .insert(user.id().clone(), password_hash);
        if let Some(session) = &session {
            store
                .sessions
                .insert(session.token().to_owned(), session.clone());
        }
        store.users.insert(user.id().clone(), user.clone());
        Ok((user, session))
    }

    pub fn login(
        &self,
        login_name: &str,
        password: &str,
    ) -> Result<(User, Session), ApplicationError> {
        let user = self.verify_login(login_name, password)?;
        let session = new_session(user.id().clone())?;
        let mut store = self.write_store()?;
        // Don't revoke old sessions here — that happens later when the
        // user clicks "进入游戏" (revoke_other_sessions is called).
        if let Some(identity_store) = &self.identity_store {
            identity_store.insert_session_only(&session)?;
        }
        store
            .sessions
            .insert(session.token().to_owned(), session.clone());
        Ok((user, session))
    }

    /// Kicks out every other session for this user.
    ///
    /// Called when the user clicks "进入游戏" so that an earlier tab is
    /// force-logged-out in real time.
    pub fn revoke_other_sessions(
        &self,
        user_id: &UserId,
        keep_token: &str,
    ) -> Result<(), ApplicationError> {
        let mut store = self.write_store()?;
        store
            .sessions
            .retain(|token, session| session.user_id() != user_id || token == keep_token);
        if let Some(identity_store) = &self.identity_store {
            identity_store.revoke_other_sessions(user_id, keep_token)?;
        }
        Ok(())
    }

    pub fn verify_login(&self, login_name: &str, password: &str) -> Result<User, ApplicationError> {
        let login_name = canonical_login_name(login_name)?;
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(invalid_credentials());
        }
        let credentials = {
            let store = self.read_store()?;
            store.login_index.get(&login_name).map(|user_id| {
                let password_hash = store.password_hashes.get(user_id).cloned();
                let user = store.users.get(user_id).cloned();
                (user_id.clone(), password_hash, user)
            })
        };
        let Some((_user_id, password_hash, user)) = credentials else {
            verify_unknown_credentials(password)?;
            return Err(invalid_credentials());
        };
        let password_hash = password_hash.ok_or_else(internal_error)?;
        let user = user.ok_or_else(internal_error)?;
        verify_password(password, &password_hash)?;
        if user.status() != AccountStatus::Active {
            return Err(ApplicationError::new(
                ErrorCode::UserUnavailable,
                "user account is unavailable",
            ));
        }
        Ok(user)
    }

    pub fn authenticate(&self, token: &str) -> Result<User, ApplicationError> {
        let store = self.read_store()?;
        let session = store.sessions.get(token).ok_or_else(|| {
            ApplicationError::new(ErrorCode::InvalidSession, "session is invalid")
        })?;
        store
            .users
            .get(session.user_id())
            .filter(|user| user.status() == AccountStatus::Active)
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(ErrorCode::UserUnavailable, "user account is unavailable")
            })
    }

    pub fn update_profile(
        &self,
        actor: &UserId,
        command: UpdateProfile,
    ) -> Result<User, ApplicationError> {
        let nickname = Nickname::parse(command.nickname)?;
        let mut store = self.write_store()?;
        let mut user = store.users.get(actor).cloned().ok_or_else(internal_error)?;
        user.rename(nickname);
        if let Some(identity_store) = &self.identity_store {
            identity_store.update_user(&user, false)?;
        }
        store.users.insert(actor.clone(), user.clone());
        Ok(user)
    }

    pub fn update_presentation(
        &self,
        actor: &UserId,
        command: UpdatePresentation,
    ) -> Result<User, ApplicationError> {
        let mut store = self.write_store()?;
        let character = store
            .characters
            .get(&command.character_id)
            .filter(|character| character.enabled())
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::InvalidCharacter,
                    "selected character is unavailable",
                )
            })?;
        if !character
            .emotes()
            .iter()
            .any(|avatar| avatar.path == command.avatar_path)
        {
            return Err(ApplicationError::new(
                ErrorCode::InvalidCharacter,
                "selected avatar does not belong to the character",
            ));
        }
        if !character
            .outfits()
            .iter()
            .any(|outfit| outfit.id == command.outfit_id)
        {
            return Err(ApplicationError::new(
                ErrorCode::InvalidCharacter,
                "selected outfit does not belong to the character",
            ));
        }
        let mut user = store.users.get(actor).cloned().ok_or_else(internal_error)?;
        user.select_presentation(
            crate::CharacterSummary::new(character.id().to_owned(), character.name().to_owned()),
            command.outfit_id,
            command.avatar_path,
        );
        if let Some(identity_store) = &self.identity_store {
            identity_store.update_user(&user, false)?;
        }
        store.users.insert(actor.clone(), user.clone());
        Ok(user)
    }

    pub fn update_tablecloth(
        &self,
        actor: &UserId,
        command: UpdateTablecloth,
    ) -> Result<User, ApplicationError> {
        let mut store = self.write_store()?;
        let tablecloth = store
            .tablecloths
            .get(&command.tablecloth_id)
            .filter(|tablecloth| tablecloth.enabled())
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::InvalidTablecloth,
                    "selected tablecloth is unavailable",
                )
            })?;
        let tablecloth_id = tablecloth.id().to_owned();
        let mut user = store.users.get(actor).cloned().ok_or_else(internal_error)?;
        user.select_tablecloth(tablecloth_id);
        if let Some(identity_store) = &self.identity_store {
            identity_store.update_user(&user, false)?;
        }
        store.users.insert(actor.clone(), user.clone());
        Ok(user)
    }

    pub fn update_music(
        &self,
        actor: &UserId,
        command: UpdateMusic,
    ) -> Result<User, ApplicationError> {
        let mut store = self.write_store()?;
        let mut selections = Vec::new();
        let mut riichi_clear = false;
        for (scene, track_id) in [
            (MusicScene::Lobby, command.lobby_music_id),
            (MusicScene::Match, command.match_music_id),
            (MusicScene::Riichi, command.riichi_music_id.clone()),
        ] {
            let Some(track_id) = track_id else {
                continue;
            };
            // 立直音乐可以传空串来清除选择。
            if scene == MusicScene::Riichi && track_id.is_empty() {
                riichi_clear = true;
                continue;
            }
            let track = store
                .music_tracks
                .get(&track_id)
                .filter(|track| track.enabled() && track.scene() == scene)
                .ok_or_else(|| {
                    ApplicationError::new(
                        ErrorCode::InvalidMusicTrack,
                        "selected music track is unavailable",
                    )
                })?;
            selections.push((scene, track.id().to_owned()));
        }
        let mut user = store.users.get(actor).cloned().ok_or_else(internal_error)?;
        for (scene, track_id) in selections {
            user.select_music(scene, track_id);
        }
        if riichi_clear {
            user.profile_mut().clear_riichi_music();
            user.increment_version();
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.update_user(&user, false)?;
        }
        store.users.insert(actor.clone(), user.clone());
        Ok(user)
    }

    pub fn user(&self, user_id: &UserId) -> Result<User, ApplicationError> {
        self.read_store()?
            .users
            .get(user_id)
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(ErrorCode::UserUnavailable, "user account was not found")
            })
    }

    pub fn list_users(&self) -> Result<Vec<User>, ApplicationError> {
        let mut users: Vec<_> = self.read_store()?.users.values().cloned().collect();
        users.sort_unstable_by(|left, right| left.login_name().cmp(right.login_name()));
        Ok(users)
    }

    pub fn set_user_status(
        &self,
        user_id: &UserId,
        status: AccountStatus,
    ) -> Result<User, ApplicationError> {
        let mut store = self.write_store()?;
        let mut user = store.users.get(user_id).cloned().ok_or_else(|| {
            ApplicationError::new(ErrorCode::UserUnavailable, "user account was not found")
        })?;
        user.set_status(status);
        if let Some(identity_store) = &self.identity_store {
            identity_store.update_user(&user, status == AccountStatus::Suspended)?;
        }
        store.users.insert(user_id.clone(), user.clone());
        if status == AccountStatus::Suspended {
            store
                .sessions
                .retain(|_, session| session.user_id() != user_id);
        }
        Ok(user)
    }

    pub fn list_characters(&self) -> Result<Vec<Character>, ApplicationError> {
        let mut characters: Vec<_> = self.read_store()?.characters.values().cloned().collect();
        characters.sort_unstable_by(|left, right| left.name().cmp(right.name()));
        Ok(characters)
    }

    pub fn default_character(&self) -> Result<Character, ApplicationError> {
        let store = self.read_store()?;
        store
            .characters
            .values()
            .find(|character| character.enabled() && character.is_default())
            .or_else(|| {
                store
                    .characters
                    .values()
                    .find(|character| character.enabled())
            })
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(ErrorCode::CharacterNotFound, "no enabled character exists")
            })
    }

    pub fn save_character(&self, command: SaveCharacter) -> Result<Character, ApplicationError> {
        let mut store = self.write_store()?;
        let removing_only_default = store
            .characters
            .get(&command.id)
            .is_some_and(Character::is_default)
            && !command.is_default
            && !store
                .characters
                .values()
                .any(|character| character.id() != command.id && character.is_default());
        if removing_only_default {
            return Err(ApplicationError::new(
                ErrorCode::CharacterDefaultRequired,
                "another enabled default character must be selected first",
            ));
        }
        let version = store
            .characters
            .get(&command.id)
            .map_or(1, |character| character.version().saturating_add(1));
        let character = Character::from_command(command, version)?;
        if character.is_default() {
            for existing in store.characters.values_mut() {
                existing.clear_default();
            }
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.upsert_character(&character)?;
        }
        store
            .characters
            .insert(character.id().to_owned(), character.clone());
        Ok(character)
    }

    pub fn delete_character(&self, character_id: &str) -> Result<(), ApplicationError> {
        let mut store = self.write_store()?;
        let character = store.characters.get(character_id).ok_or_else(|| {
            ApplicationError::new(ErrorCode::CharacterNotFound, "character was not found")
        })?;
        if character.is_default() {
            return Err(ApplicationError::new(
                ErrorCode::CharacterDefaultRequired,
                "the default character cannot be deleted",
            ));
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.delete_character(character_id)?;
        }
        store.characters.remove(character_id);
        Ok(())
    }

    pub fn list_tablecloths(&self) -> Result<Vec<Tablecloth>, ApplicationError> {
        let mut tablecloths: Vec<_> = self.read_store()?.tablecloths.values().cloned().collect();
        tablecloths.sort_unstable_by(|left, right| left.name().cmp(right.name()));
        Ok(tablecloths)
    }

    pub fn default_tablecloth(&self) -> Result<Tablecloth, ApplicationError> {
        let store = self.read_store()?;
        store
            .tablecloths
            .values()
            .find(|tablecloth| tablecloth.enabled() && tablecloth.is_default())
            .or_else(|| {
                store
                    .tablecloths
                    .values()
                    .find(|tablecloth| tablecloth.enabled())
            })
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::TableclothNotFound,
                    "no enabled tablecloth exists",
                )
            })
    }

    pub fn save_tablecloth(&self, command: SaveTablecloth) -> Result<Tablecloth, ApplicationError> {
        let mut store = self.write_store()?;
        let removing_only_default = store
            .tablecloths
            .get(&command.id)
            .is_some_and(Tablecloth::is_default)
            && !command.is_default
            && !store
                .tablecloths
                .values()
                .any(|tablecloth| tablecloth.id() != command.id && tablecloth.is_default());
        if removing_only_default {
            return Err(ApplicationError::new(
                ErrorCode::TableclothDefaultRequired,
                "another enabled default tablecloth must be selected first",
            ));
        }
        let version = store
            .tablecloths
            .get(&command.id)
            .map_or(1, |tablecloth| tablecloth.version().saturating_add(1));
        let tablecloth = Tablecloth::from_command(command, version)?;
        if tablecloth.is_default() {
            for existing in store.tablecloths.values_mut() {
                existing.clear_default();
            }
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.upsert_tablecloth(&tablecloth)?;
        }
        store
            .tablecloths
            .insert(tablecloth.id().to_owned(), tablecloth.clone());
        Ok(tablecloth)
    }

    pub fn delete_tablecloth(&self, tablecloth_id: &str) -> Result<(), ApplicationError> {
        let mut store = self.write_store()?;
        let tablecloth = store.tablecloths.get(tablecloth_id).ok_or_else(|| {
            ApplicationError::new(ErrorCode::TableclothNotFound, "tablecloth was not found")
        })?;
        if tablecloth.is_default() {
            return Err(ApplicationError::new(
                ErrorCode::TableclothDefaultRequired,
                "the default tablecloth cannot be deleted",
            ));
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.delete_tablecloth(tablecloth_id)?;
        }
        store.tablecloths.remove(tablecloth_id);
        Ok(())
    }

    pub fn list_music_tracks(&self) -> Result<Vec<MusicTrack>, ApplicationError> {
        let mut tracks: Vec<_> = self.read_store()?.music_tracks.values().cloned().collect();
        tracks.sort_unstable_by(|left, right| left.name().cmp(right.name()));
        Ok(tracks)
    }

    pub fn default_music_track(&self, scene: MusicScene) -> Result<MusicTrack, ApplicationError> {
        let store = self.read_store()?;
        store
            .music_tracks
            .values()
            .find(|track| track.scene() == scene && track.enabled() && track.is_default())
            .or_else(|| {
                store
                    .music_tracks
                    .values()
                    .find(|track| track.scene() == scene && track.enabled())
            })
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(
                    ErrorCode::MusicTrackNotFound,
                    "no enabled music track exists for this scene",
                )
            })
    }

    pub fn save_music_track(
        &self,
        command: SaveMusicTrack,
    ) -> Result<MusicTrack, ApplicationError> {
        let mut store = self.write_store()?;
        let existing = store.music_tracks.get(&command.id);
        // 同一场景里必须始终留着一首默认曲，否则新玩家没有音乐可放。
        let removing_only_default = existing.is_some_and(|track| {
            track.is_default()
                && (!command.is_default || track.scene() != command.scene)
                && !store.music_tracks.values().any(|other| {
                    other.id() != command.id && other.scene() == track.scene() && other.is_default()
                })
        });
        if removing_only_default {
            return Err(ApplicationError::new(
                ErrorCode::MusicTrackDefaultRequired,
                "another default music track must be selected for this scene first",
            ));
        }
        let version = existing.map_or(1, |track| track.version().saturating_add(1));
        let track = MusicTrack::from_command(command, version)?;
        if track.is_default() {
            for existing in store
                .music_tracks
                .values_mut()
                .filter(|existing| existing.scene() == track.scene())
            {
                existing.clear_default();
            }
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.upsert_music_track(&track)?;
        }
        store
            .music_tracks
            .insert(track.id().to_owned(), track.clone());
        Ok(track)
    }

    pub fn delete_music_track(&self, track_id: &str) -> Result<(), ApplicationError> {
        let mut store = self.write_store()?;
        let track = store.music_tracks.get(track_id).ok_or_else(|| {
            ApplicationError::new(ErrorCode::MusicTrackNotFound, "music track was not found")
        })?;
        if track.is_default() {
            return Err(ApplicationError::new(
                ErrorCode::MusicTrackDefaultRequired,
                "the default music track cannot be deleted",
            ));
        }
        if let Some(identity_store) = &self.identity_store {
            identity_store.delete_music_track(track_id)?;
        }
        store.music_tracks.remove(track_id);
        Ok(())
    }

    pub fn create_room(
        &self,
        actor: &UserId,
        command: CreateRoom,
    ) -> Result<Room, ApplicationError> {
        let name = validate_room_name(command.name)?;
        let snapshot = resolve_rules(command.rules)?;
        let mut store = self.write_store()?;
        eject_user_from_lobby(&mut store, actor, None)?;
        let owner = store.users.get(actor).ok_or_else(internal_error)?;
        let room_id = unique_room_id(&store)?;
        let room = Room::new(
            room_id,
            actor.clone(),
            owner.profile().nickname().as_str().to_owned(),
            name,
            command.visibility,
            snapshot,
            false,
        );
        store.rooms.insert(room.id().clone(), room.clone());
        Ok(room)
    }

    pub fn list_rooms(&self) -> Result<Vec<Room>, ApplicationError> {
        let store = self.read_store()?;
        let mut rooms: Vec<_> = store
            .rooms
            .values()
            .filter(|room| {
                room.visibility() == RoomVisibility::Public
                    && room.lifecycle() == crate::RoomLifecycle::Waiting
            })
            .cloned()
            .collect();
        rooms.sort_unstable_by(|left, right| left.id().cmp(right.id()));
        Ok(rooms)
    }

    pub fn list_all_rooms(&self) -> Result<Vec<Room>, ApplicationError> {
        let mut rooms: Vec<_> = self.read_store()?.rooms.values().cloned().collect();
        rooms.sort_unstable_by(|left, right| left.id().cmp(right.id()));
        Ok(rooms)
    }

    pub fn current_room(&self, actor: &UserId) -> Result<Option<Room>, ApplicationError> {
        Ok(self
            .read_store()?
            .rooms
            .values()
            .find(|room| {
                room.lifecycle() != crate::RoomLifecycle::Closed
                    && room
                        .members()
                        .iter()
                        .any(|member| member.user_id() == actor)
            })
            .cloned())
    }

    pub fn current_matchmaking_ticket(
        &self,
        actor: &UserId,
    ) -> Result<Option<MatchmakingTicket>, ApplicationError> {
        Ok(self
            .read_store()?
            .matchmaking_tickets
            .values()
            .find(|ticket| {
                ticket.user_id() == actor && matches!(ticket.status(), MatchmakingStatus::Waiting)
            })
            .cloned())
    }

    pub fn room(&self, room_id: &RoomId) -> Result<Room, ApplicationError> {
        self.read_store()?
            .rooms
            .get(room_id)
            .cloned()
            .ok_or_else(room_not_found)
    }

    pub fn close_room_by_administrator(&self, room_id: &RoomId) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
        let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
        room.close_by_administrator()?;
        Ok(room.clone())
    }

    pub fn join_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
    ) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
        eject_user_from_lobby(&mut store, actor, Some(room_id))?;
        let nickname = store
            .users
            .get(actor)
            .ok_or_else(internal_error)?
            .profile()
            .nickname()
            .as_str()
            .to_owned();
        let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
        ensure_version(room, expected_version)?;
        room.join(actor.clone(), nickname)?;
        Ok(room.clone())
    }

    pub fn set_ready(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
        ready: bool,
    ) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
        let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
        ensure_version(room, expected_version)?;
        room.set_ready(actor, ready)?;
        Ok(room.clone())
    }

    pub fn update_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        command: UpdateRoom,
    ) -> Result<Room, ApplicationError> {
        let name = command.name.map(validate_room_name).transpose()?;
        let rules = command.rules.map(resolve_rules).transpose()?;
        let mut store = self.write_store()?;
        let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
        ensure_version(room, command.expected_version)?;
        room.update(actor, name, command.visibility, rules)?;
        Ok(room.clone())
    }

    pub fn leave_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
    ) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
        let departed = {
            let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
            ensure_version(room, expected_version)?;
            room.leave(actor)?;
            room.clone()
        };
        if departed.members().is_empty() {
            store.rooms.remove(room_id);
        }
        Ok(departed)
    }

    pub fn leave_room_current(
        &self,
        actor: &UserId,
        room_id: &RoomId,
    ) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
        let departed = {
            let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
            room.leave(actor)?;
            room.clone()
        };
        if departed.members().is_empty() {
            store.rooms.remove(room_id);
        }
        Ok(departed)
    }

    pub fn start_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
        now_ms: u64,
    ) -> Result<(Room, MatchId), ApplicationError> {
        let mut store = self.write_store()?;
        let mut room = store.rooms.get(room_id).ok_or_else(room_not_found)?.clone();
        ensure_version(&room, expected_version)?;
        let match_id = room.start(actor)?;
        let game = GameRuntime::start(&room, match_id.clone(), now_ms)?;
        store.rooms.insert(room_id.clone(), room.clone());
        store.matches.insert(match_id.clone(), game);
        Ok((room, match_id))
    }

    /// 按规则家族分叉的牌桌投影，服务端 DTO 走这条。
    pub fn match_projection(
        &self,
        actor: &UserId,
        match_id: &MatchId,
    ) -> Result<MatchProjection, ApplicationError> {
        self.read_store()?
            .matches
            .get(match_id)
            .ok_or_else(match_not_found)?
            .projection(actor)
    }

    /// 只认立直投影的调用方（控制台客户端、bot、现有测试）走这条。
    ///
    /// # Errors
    ///
    /// 这局不是立直麻将时返回 `InvalidGameCommand`。
    pub fn match_view(
        &self,
        actor: &UserId,
        match_id: &MatchId,
    ) -> Result<ObserverMatch, ApplicationError> {
        self.match_projection(actor, match_id)?.into_riichi()
    }

    pub fn match_record(
        &self,
        actor: &UserId,
        match_id: &MatchId,
    ) -> Result<crate::MatchRecord, ApplicationError> {
        let store = self.read_store()?;
        let game = store.matches.get(match_id).ok_or_else(match_not_found)?;
        // 冲击麻将本期不生成牌谱，调用方会拿到一个明确的错误而不是半张牌谱。
        crate::MatchRecord::from_runtime(game.as_riichi().ok_or_else(not_riichi)?, actor)
    }

    /// 这局会不会出牌谱。
    ///
    /// 归档在每一步之后都要落盘，而冲击麻将本期没有牌谱；调用方先问一句再决定跳过，
    /// 免得把「这套规则不出牌谱」当成「归档写失败」而让整步操作报 500。
    /// 对局不存在时同样返回 `false`：没有东西可归档。
    #[must_use]
    pub fn match_generates_record(&self, match_id: &MatchId) -> bool {
        self.read_store()
            .ok()
            .and_then(|store| {
                store
                    .matches
                    .get(match_id)
                    .map(GameRuntime::generates_record)
            })
            .unwrap_or(false)
    }

    pub fn match_events(
        &self,
        actor: &UserId,
        match_id: &MatchId,
        after_sequence: u64,
    ) -> Result<MatchEventPage, ApplicationError> {
        self.read_store()?
            .matches
            .get(match_id)
            .ok_or_else(match_not_found)?
            .events_after(actor, after_sequence)
    }

    pub fn submit_game(
        &self,
        actor: &UserId,
        match_id: &MatchId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<MatchProjection, ApplicationError> {
        let mut store = self.write_store()?;
        let view = {
            let game = store
                .matches
                .get_mut(match_id)
                .ok_or_else(match_not_found)?;
            game.execute(actor, command, now_ms)?;
            game.projection(actor)?
        };
        if view.has_result() || view.terminated_by_exit_vote() {
            if let Some(room) = store
                .rooms
                .values_mut()
                .find(|room| room.active_match_id() == Some(match_id))
            {
                room.finish_match(match_id)?;
            }
        }
        Ok(view)
    }

    /// 开发/测试专用：把某位玩家的 13 张暗手整体替换成给定牌码。只有开着的服务器才该调。
    pub fn set_dev_hand(
        &self,
        actor: &UserId,
        match_id: &MatchId,
        codes: &[String],
    ) -> Result<MatchProjection, ApplicationError> {
        let mut store = self.write_store()?;
        let game = store
            .matches
            .get_mut(match_id)
            .ok_or_else(match_not_found)?;
        game.set_dev_hand(actor, codes)?;
        game.projection(actor)
    }

    /// 只认立直投影的调用方走这条。
    ///
    /// # Errors
    ///
    /// 这局不是立直麻将时返回 `InvalidGameCommand`。
    pub fn submit_game_command(
        &self,
        actor: &UserId,
        match_id: &MatchId,
        command: SubmitGameCommand,
        now_ms: u64,
    ) -> Result<ObserverMatch, ApplicationError> {
        self.submit_game(actor, match_id, command, now_ms)?
            .into_riichi()
    }

    /// Plays the timeout action for every seat whose clock ran out.
    ///
    /// Advances each match at most once per call so a single sweep cannot run
    /// a whole hand; the remaining expiries are handled by the next sweep.
    pub fn expire_clocks(&self, now_ms: u64) -> Result<Vec<ClockExpiry>, ApplicationError> {
        let mut store = self.write_store()?;
        let candidates = store
            .matches
            .iter()
            .filter(|(_, game)| !game.is_finished() || game.has_pending_settlement())
            .map(|(match_id, _)| match_id.clone())
            .collect::<Vec<_>>();
        let mut expiries = Vec::new();
        for match_id in candidates {
            let game = store
                .matches
                .get_mut(&match_id)
                .ok_or_else(match_not_found)?;
            // 有人一直没把对局素材load完，整局作废，各家回房间。
            if game.terminate_if_assets_stalled(now_ms)? {
                let actor = game.any_player().ok_or_else(internal_error)?;
                let expiry = ClockExpiry {
                    match_id: match_id.clone(),
                    actor,
                    version: game.version(),
                    latest_sequence: game.event_sequence(),
                    finished: true,
                };
                let room = store
                    .rooms
                    .values_mut()
                    .find(|room| room.active_match_id() == Some(&match_id))
                    .ok_or_else(internal_error)?;
                room.finish_match(&match_id)?;
                expiries.push(expiry);
                continue;
            }
            // 这三条都是服务端替客户端做的放行，都必须广播出去，客户端在收到新
            // 版本之前一直按着不动。
            let released = game.advance_settlement_if_due(now_ms)?
                || game.open_settlement_confirm_if_due(now_ms)?
                || game.release_opening_if_due(now_ms)?;
            if released {
                let actor = game.any_player().ok_or_else(internal_error)?;
                expiries.push(ClockExpiry {
                    match_id: match_id.clone(),
                    actor,
                    version: game.version(),
                    latest_sequence: game.event_sequence(),
                    finished: game.is_finished(),
                });
                continue;
            }
            let Some(actor) = game.expire(now_ms)? else {
                continue;
            };
            let expiry = ClockExpiry {
                match_id: match_id.clone(),
                actor,
                version: game.version(),
                latest_sequence: game.event_sequence(),
                finished: game.is_finished(),
            };
            if expiry.finished {
                let room = store
                    .rooms
                    .values_mut()
                    .find(|room| room.active_match_id() == Some(&match_id))
                    .ok_or_else(internal_error)?;
                room.finish_match(&match_id)?;
            }
            expiries.push(expiry);
        }
        Ok(expiries)
    }

    pub fn enter_matchmaking(
        &self,
        actor: &UserId,
        variant: RiichiVariant,
        now_ms: u64,
    ) -> Result<MatchmakingTicket, ApplicationError> {
        let mut store = self.write_store()?;
        eject_user_from_lobby(&mut store, actor, None)?;
        if !store.users.contains_key(actor) {
            return Err(internal_error());
        }
        let join_order = store.next_matchmaking_order;
        store.next_matchmaking_order = store
            .next_matchmaking_order
            .checked_add(1)
            .ok_or_else(internal_error)?;
        let ticket = MatchmakingTicket::new(actor.clone(), variant, join_order);

        let mut candidates: Vec<_> = store
            .matchmaking_tickets
            .values()
            .filter(|candidate| {
                candidate.variant() == variant
                    && matches!(candidate.status(), MatchmakingStatus::Waiting)
            })
            .cloned()
            .collect();
        candidates.push(ticket.clone());
        candidates.sort_unstable_by_key(|candidate| candidate.join_order);
        let required = usize::from(variant.seat_count().value());
        if candidates.len() < required {
            store
                .matchmaking_tickets
                .insert(ticket.id().clone(), ticket.clone());
            return Ok(ticket);
        }
        let selected = &candidates[..required];
        let mut selected_users = Vec::with_capacity(required);
        for candidate in selected {
            let user = store
                .users
                .get(candidate.user_id())
                .ok_or_else(internal_error)?;
            selected_users.push((
                candidate.id().clone(),
                candidate.user_id().clone(),
                user.profile().nickname().as_str().to_owned(),
            ));
        }

        let snapshot = RiichiRuleSnapshot::try_new(RiichiRules::standard(variant), None)
            .map_err(|_| internal_error())?;
        let room_id = unique_room_id(&store)?;
        let mut room = Room::new(
            room_id,
            selected_users[0].1.clone(),
            selected_users[0].2.clone(),
            format!(
                "段位{}",
                if matches!(variant, RiichiVariant::Yonma) {
                    "四麻"
                } else {
                    "三麻"
                }
            ),
            RoomVisibility::Private,
            GameRuleSnapshot::Riichi(snapshot),
            true,
        );
        for (_, user_id, nickname) in &selected_users[1..] {
            room.join(user_id.clone(), nickname.clone())?;
        }
        for (_, user_id, _) in &selected_users {
            room.set_ready(user_id, true)?;
        }
        let match_id = room.start(&selected_users[0].1)?;
        let game = GameRuntime::start(&room, match_id.clone(), now_ms)?;
        let room_id = room.id().clone();

        store
            .matchmaking_tickets
            .insert(ticket.id().clone(), ticket);
        for (ticket_id, _, _) in &selected_users {
            store
                .matchmaking_tickets
                .get_mut(ticket_id)
                .ok_or_else(internal_error)?
                .mark_matched(room_id.clone(), match_id.clone());
        }
        store.rooms.insert(room_id, room);
        store.matches.insert(match_id, game);
        store
            .matchmaking_tickets
            .get(
                selected
                    .iter()
                    .find(|candidate| candidate.user_id() == actor)
                    .ok_or_else(internal_error)?
                    .id(),
            )
            .cloned()
            .ok_or_else(internal_error)
    }

    pub fn matchmaking_ticket(
        &self,
        actor: &UserId,
        ticket_id: &TicketId,
    ) -> Result<MatchmakingTicket, ApplicationError> {
        self.read_store()?
            .matchmaking_tickets
            .get(ticket_id)
            .filter(|ticket| ticket.user_id() == actor)
            .cloned()
            .ok_or_else(matchmaking_ticket_not_found)
    }

    pub fn cancel_matchmaking(
        &self,
        actor: &UserId,
        ticket_id: &TicketId,
    ) -> Result<MatchmakingTicket, ApplicationError> {
        let mut store = self.write_store()?;
        let ticket = store
            .matchmaking_tickets
            .get_mut(ticket_id)
            .filter(|ticket| ticket.user_id() == actor)
            .ok_or_else(matchmaking_ticket_not_found)?;
        if !matches!(ticket.status(), MatchmakingStatus::Waiting) {
            return Err(ApplicationError::new(
                ErrorCode::MatchmakingTicketNotWaiting,
                "matchmaking ticket is no longer waiting",
            ));
        }
        ticket.cancel();
        Ok(ticket.clone())
    }

    fn read_store(&self) -> Result<RwLockReadGuard<'_, MemoryStore>, ApplicationError> {
        self.store.read().map_err(|_| internal_error())
    }

    fn write_store(&self) -> Result<RwLockWriteGuard<'_, MemoryStore>, ApplicationError> {
        self.store.write().map_err(|_| internal_error())
    }
}

impl Default for Application {
    fn default() -> Self {
        let character = Character::from_command(ichihime_default(), 1)
            .expect("built-in character metadata must be valid");
        let mut store = MemoryStore::default();
        store
            .characters
            .insert(character.id().to_owned(), character);
        for command in built_in_tablecloths() {
            let tablecloth = Tablecloth::from_command(command, 1)
                .expect("built-in tablecloth metadata must be valid");
            store
                .tablecloths
                .insert(tablecloth.id().to_owned(), tablecloth);
        }
        for command in built_in_music_tracks() {
            let track = MusicTrack::from_command(command, 1)
                .expect("built-in music metadata must be valid");
            store.music_tracks.insert(track.id().to_owned(), track);
        }
        Self {
            store: Arc::new(RwLock::new(store)),
            identity_store: None,
        }
    }
}

fn canonical_login_name(value: &str) -> Result<String, ApplicationError> {
    let value = value.trim();
    let valid_length = (3..=32).contains(&value.len());
    let mut characters = value.chars();
    let valid_start = characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric());
    let valid_rest =
        characters.all(|character| character.is_ascii_alphanumeric() || "_-".contains(character));
    if !valid_length || !valid_start || !valid_rest || !value.is_ascii() {
        return Err(ApplicationError::new(
            ErrorCode::InvalidLoginName,
            "login name must be 3 to 32 ASCII letters, digits, underscores, or hyphens",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_password_with_minimum(
    password: &str,
    minimum_password_bytes: usize,
) -> Result<(), ApplicationError> {
    if !(minimum_password_bytes..=MAX_PASSWORD_BYTES).contains(&password.len()) {
        return Err(ApplicationError::new(
            ErrorCode::InvalidPassword,
            format!("password must contain {minimum_password_bytes} to 128 bytes"),
        ));
    }
    Ok(())
}

fn verify_unknown_credentials(password: &str) -> Result<(), ApplicationError> {
    let encoded = DUMMY_PASSWORD_HASH
        .get_or_init(|| hash_password("mamahjong dummy credential"))
        .as_ref()
        .map_err(|_| internal_error())?;
    let _ = verify_password(password, encoded);
    Ok(())
}

fn validate_room_name(value: String) -> Result<String, ApplicationError> {
    let value = value.trim();
    if !(1..=40).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(ApplicationError::new(
            ErrorCode::InvalidRoomName,
            "room name must contain 1 to 40 non-control characters",
        ));
    }
    Ok(value.to_owned())
}

fn unique_room_id(store: &MemoryStore) -> Result<RoomId, ApplicationError> {
    for _ in 0..64 {
        let room_id = RoomId::new();
        if !store.rooms.contains_key(&room_id) {
            return Ok(room_id);
        }
    }
    Err(internal_error())
}

fn hash_password(password: &str) -> Result<String, ApplicationError> {
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|_| internal_error())?;
    let salt = SaltString::encode_b64(&salt).map_err(|_| internal_error())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| internal_error())
}

fn verify_password(password: &str, encoded: &str) -> Result<(), ApplicationError> {
    let encoded = PasswordHash::new(encoded).map_err(|_| internal_error())?;
    Argon2::default()
        .verify_password(password.as_bytes(), &encoded)
        .map_err(|_| invalid_credentials())
}

fn new_session(user_id: UserId) -> Result<Session, ApplicationError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| internal_error())?;
    let mut token = String::with_capacity(67);
    token.push_str("mj_");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(Session::new(user_id, token))
}

fn resolve_rules(selection: RoomRuleSelection) -> Result<GameRuleSnapshot, ApplicationError> {
    match selection {
        RoomRuleSelection::Riichi { variant, request } => request
            .resolve_snapshot(variant)
            .map(GameRuleSnapshot::Riichi)
            .map_err(|error| {
                ApplicationError::new(ErrorCode::InvalidRuleConfiguration, error.to_string())
            }),
        RoomRuleSelection::Impact { request } => request
            .resolve_snapshot()
            .map(GameRuleSnapshot::Impact)
            .map_err(|error| {
                ApplicationError::new(ErrorCode::InvalidRuleConfiguration, error.to_string())
            }),
    }
}

fn ensure_version(room: &Room, expected: u64) -> Result<(), ApplicationError> {
    if room.version() == expected {
        Ok(())
    } else {
        Err(ApplicationError::new(
            ErrorCode::RoomVersionConflict,
            format!(
                "expected room version {expected}, current version is {}",
                room.version()
            ),
        ))
    }
}

fn eject_user_from_lobby(
    store: &mut MemoryStore,
    actor: &UserId,
    except_room_id: Option<&RoomId>,
) -> Result<(), ApplicationError> {
    // 自动退出老房间（Waiting 状态的），跳过正要加入的那一间。
    let rooms_to_leave: Vec<RoomId> = store
        .rooms
        .iter()
        .filter(|(_, room)| {
            let is_exception = except_room_id.is_some_and(|eid| room.id() == eid);
            !is_exception
                && room.lifecycle() == crate::RoomLifecycle::Waiting
                && room.members().iter().any(|m| m.user_id() == actor)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for room_id in &rooms_to_leave {
        if let Some(room) = store.rooms.get_mut(room_id) {
            let _ = room.leave(actor);
        }
    }
    store.rooms.retain(|_, room| !room.members().is_empty());

    // 取消还在等待的匹配队列。
    for ticket in store.matchmaking_tickets.values_mut() {
        if ticket.user_id() == actor && matches!(ticket.status(), MatchmakingStatus::Waiting) {
            ticket.cancel();
        }
    }

    // 如果用户还在 Playing 房间（不能退出），回报错误。
    let stuck = store.rooms.values().any(|room| {
        let is_exception = except_room_id.is_some_and(|eid| room.id() == eid);
        !is_exception
            && room.lifecycle() != crate::RoomLifecycle::Closed
            && room.members().iter().any(|m| m.user_id() == actor)
    });
    if stuck {
        return Err(ApplicationError::new(
            ErrorCode::UserBusy,
            "user is in an active game and cannot leave",
        ));
    }
    Ok(())
}

fn invalid_credentials() -> ApplicationError {
    ApplicationError::new(
        ErrorCode::InvalidCredentials,
        "login name or password is invalid",
    )
}

fn room_not_found() -> ApplicationError {
    ApplicationError::new(ErrorCode::RoomNotFound, "room was not found")
}

fn match_not_found() -> ApplicationError {
    ApplicationError::new(ErrorCode::MatchNotFound, "match was not found")
}

fn matchmaking_ticket_not_found() -> ApplicationError {
    ApplicationError::new(
        ErrorCode::MatchmakingTicketNotFound,
        "matchmaking ticket was not found",
    )
}

fn internal_error() -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, "internal application state error")
}

#[cfg(test)]
mod tests {
    use mahjong_riichi::{
        AbortiveDrawRuleOverrides, DealerContinuation, MatchLength, MatchRuleOverrides,
        RiichiRuleOverrides, RiichiVariant, RonResolution, ScoringRuleOverrides,
        SettlementRuleOverrides,
    };

    use super::{
        Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdatePresentation,
        UpdateProfile, UpdateRoom, UpdateTablecloth,
    };
    use crate::{
        AccountRole, AccountStatus, ErrorCode, GameCommand, MatchmakingStatus, MusicScene,
        RoomVisibility, SaveMusicTrack, SubmitGameCommand,
    };

    /// 结算的两段握手：先报告动画播完，服务端开了确认窗口才收得下确认。
    const SETTLEMENT_HANDSHAKE: [fn(u32) -> GameCommand; 2] = [
        |hand_index| GameCommand::SettlementPlayed { hand_index },
        |hand_index| GameCommand::ConfirmSettlement { hand_index },
    ];

    fn register(application: &Application, suffix: &str) -> (crate::User, crate::Session) {
        application
            .register(RegisterUser {
                login_name: format!("player_{suffix}"),
                password: "correct horse battery staple".to_owned(),
                nickname: format!("雀士{suffix}"),
            })
            .expect("register")
    }

    #[test]
    fn moving_the_only_default_music_does_not_orphan_its_original_scene() {
        let application = Application::new();
        let error = application
            .save_music_track(SaveMusicTrack {
                id: "lobby-default".to_owned(),
                name: "默认".to_owned(),
                scene: MusicScene::Match,
                audio_path: "/user-assets/music/lobby-default.mp3".to_owned(),
                duration_ms: 75_572,
                enabled: true,
                is_default: true,
            })
            .expect_err("the original scene still needs a default");

        assert_eq!(error.code(), ErrorCode::MusicTrackDefaultRequired);
        assert_eq!(
            application
                .default_music_track(MusicScene::Lobby)
                .expect("lobby default")
                .id(),
            "lobby-default"
        );
    }

    #[test]
    fn registration_hashes_password_and_returns_redacted_session() {
        let application = Application::new();
        let (user, session) = register(&application, "one");

        assert_eq!(user.profile().nickname().as_str(), "雀士one");
        assert!(!format!("{session:?}").contains(session.token()));
        let (logged_in_user, replacement_session) = application
            .login("PLAYER_ONE", "correct horse battery staple")
            .expect("login");
        assert_eq!(logged_in_user.id(), user.id());
        // Both sessions are still valid — revocation happens later.
        assert_eq!(
            application
                .authenticate(session.token())
                .expect("the original session is still valid")
                .id(),
            user.id()
        );
        assert_eq!(
            application
                .authenticate(replacement_session.token())
                .expect("the latest session is also valid")
                .id(),
            user.id()
        );
        // Kick out other sessions explicitly.
        application
            .revoke_other_sessions(user.id(), replacement_session.token())
            .expect("revoke others");
        assert_eq!(
            application
                .authenticate(session.token())
                .expect_err("the original session is now revoked")
                .code(),
            ErrorCode::InvalidSession
        );
        assert_eq!(
            application
                .authenticate(replacement_session.token())
                .expect("the latest session remains valid")
                .id(),
            user.id()
        );
        assert_eq!(
            application
                .login("player_one", "incorrect password")
                .expect_err("invalid password")
                .code(),
            ErrorCode::InvalidCredentials
        );
    }

    #[test]
    fn sanma_matchmaking_starts_when_the_third_player_enters() {
        let application = Application::new();
        let players: Vec<_> = (0..3)
            .map(|index| register(&application, &format!("queue_{index}")).0)
            .collect();

        let first = application
            .enter_matchmaking(players[0].id(), RiichiVariant::Sanma, 0)
            .expect("first ticket");
        let second = application
            .enter_matchmaking(players[1].id(), RiichiVariant::Sanma, 0)
            .expect("second ticket");
        assert!(matches!(first.status(), MatchmakingStatus::Waiting));
        assert!(matches!(second.status(), MatchmakingStatus::Waiting));

        let third = application
            .enter_matchmaking(players[2].id(), RiichiVariant::Sanma, 0)
            .expect("matched ticket");
        let MatchmakingStatus::Matched { match_id, .. } = third.status() else {
            panic!("third player must complete a sanma table");
        };
        let match_id = match_id.clone();
        for (player, ticket) in players.iter().zip([first, second, third]) {
            let current = application
                .matchmaking_ticket(player.id(), ticket.id())
                .expect("ticket");
            assert!(matches!(
                current.status(),
                MatchmakingStatus::Matched { .. }
            ));
            assert_eq!(
                application
                    .match_view(player.id(), &match_id)
                    .expect("matched game")
                    .players()
                    .len(),
                3
            );
        }
    }

    #[test]
    fn administrator_bootstrap_and_user_suspension_are_explicit() {
        let application = Application::new();
        let administrator = application
            .bootstrap_administrator(RegisterUser {
                login_name: "administrator".to_owned(),
                password: "administrator-password".to_owned(),
                nickname: "管理员".to_owned(),
            })
            .expect("administrator");
        let (player, player_session) = register(&application, "managed_player");

        assert_eq!(administrator.role(), AccountRole::Administrator);
        assert_eq!(
            application
                .bootstrap_administrator(RegisterUser {
                    login_name: "ADMINISTRATOR".to_owned(),
                    password: "rotated-admin-password".to_owned(),
                    nickname: "不会覆盖".to_owned(),
                })
                .expect("idempotent administrator bootstrap")
                .id(),
            administrator.id()
        );
        assert!(
            application
                .verify_login("administrator", "administrator-password")
                .is_err()
        );
        application
            .verify_login("administrator", "rotated-admin-password")
            .expect("rotated administrator password");
        assert_eq!(player.role(), AccountRole::Player);
        assert_eq!(application.list_users().expect("users").len(), 2);

        let suspended = application
            .set_user_status(player.id(), AccountStatus::Suspended)
            .expect("suspend");
        assert_eq!(suspended.status(), AccountStatus::Suspended);
        assert_eq!(
            application
                .authenticate(player_session.token())
                .expect_err("sessions are revoked")
                .code(),
            ErrorCode::InvalidSession
        );
    }

    #[test]
    fn duplicate_canonical_login_is_rejected() {
        let application = Application::new();
        register(&application, "two");
        let error = application
            .register(RegisterUser {
                login_name: "PLAYER_TWO".to_owned(),
                password: "a different secure password".to_owned(),
                nickname: "另一位玩家".to_owned(),
            })
            .expect_err("duplicate");

        assert_eq!(error.code(), ErrorCode::LoginNameTaken);
    }

    #[test]
    fn profile_rename_keeps_reserved_containers() {
        let application = Application::new();
        let (user, _) = register(&application, "three");
        let updated = application
            .update_profile(
                user.id(),
                UpdateProfile {
                    nickname: "新昵称".to_owned(),
                },
            )
            .expect("rename");

        assert_eq!(updated.profile().nickname().as_str(), "新昵称");
        assert!(updated.profile().equipped_title().is_none());
        assert!(updated.profile().selected_character().is_none());
        assert!(updated.profile().ranks().is_empty());
    }

    #[test]
    fn presentation_restricts_avatar_and_outfit_to_the_selected_character() {
        let application = Application::new();
        let (user, _) = register(&application, "presentation");
        let updated = application
            .update_presentation(
                user.id(),
                UpdatePresentation {
                    character_id: "ichihime".to_owned(),
                    outfit_id: "beach".to_owned(),
                    avatar_path: "/game/assets/local-characters/mahjong-soul/ichihime/emotes/8.png"
                        .to_owned(),
                },
            )
            .expect("select presentation");

        assert_eq!(updated.profile().selected_outfit_id(), Some("beach"));
        assert_eq!(
            application
                .update_presentation(
                    user.id(),
                    UpdatePresentation {
                        character_id: "ichihime".to_owned(),
                        outfit_id: "unknown".to_owned(),
                        avatar_path:
                            "/game/assets/local-characters/mahjong-soul/ichihime/emotes/8.png"
                                .to_owned(),
                    },
                )
                .expect_err("foreign outfit")
                .code(),
            ErrorCode::InvalidCharacter
        );
        assert_eq!(
            application
                .update_presentation(
                    user.id(),
                    UpdatePresentation {
                        character_id: "ichihime".to_owned(),
                        outfit_id: "default".to_owned(),
                        avatar_path: "/game/assets/other-character/avatar.png".to_owned(),
                    },
                )
                .expect_err("foreign avatar")
                .code(),
            ErrorCode::InvalidCharacter
        );
    }

    #[test]
    fn user_can_select_only_an_enabled_catalog_tablecloth() {
        let application = Application::new();
        let (user, _) = register(&application, "tablecloth");
        let updated = application
            .update_tablecloth(
                user.id(),
                UpdateTablecloth {
                    tablecloth_id: "flowers-under-moon".to_owned(),
                },
            )
            .expect("select tablecloth");

        assert_eq!(
            updated.profile().selected_tablecloth_id(),
            Some("flowers-under-moon")
        );
        assert_eq!(
            application
                .update_tablecloth(
                    user.id(),
                    UpdateTablecloth {
                        tablecloth_id: "missing".to_owned(),
                    },
                )
                .expect_err("missing tablecloth")
                .code(),
            ErrorCode::InvalidTablecloth
        );
    }

    #[test]
    fn room_membership_versions_and_head_bump_snapshot_are_enforced() {
        let application = Application::new();
        let (owner, _) = register(&application, "owner");
        let (guest, _) = register(&application, "guest");
        let room = application
            .create_room(
                owner.id(),
                CreateRoom {
                    name: "头跳测试房".to_owned(),
                    visibility: RoomVisibility::Public,
                    rules: RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Yonma,
                        request: mahjong_riichi::RoomRuleRequest {
                            preset: None,
                            overrides: RiichiRuleOverrides {
                                settlement: Some(SettlementRuleOverrides {
                                    ron_resolution: Some(RonResolution::HeadBump),
                                    ..SettlementRuleOverrides::default()
                                }),
                                ..RiichiRuleOverrides::default()
                            },
                        },
                    },
                },
            )
            .expect("room");

        let room = application
            .join_room(guest.id(), room.id(), room.version())
            .expect("join");
        let stale = application
            .set_ready(guest.id(), room.id(), room.version() - 1, true)
            .expect_err("stale version");
        assert_eq!(stale.code(), ErrorCode::RoomVersionConflict);
        let room = application
            .set_ready(guest.id(), room.id(), room.version(), true)
            .expect("ready");
        assert!(room.members().iter().any(|member| member.ready()));
        assert_eq!(
            room.rule_snapshot()
                .as_riichi()
                .expect("riichi")
                .rules()
                .settlement
                .ron_resolution,
            RonResolution::HeadBump
        );
    }

    #[test]
    fn changing_rules_clears_readiness() {
        let application = Application::new();
        let (owner, _) = register(&application, "host");
        let (guest, _) = register(&application, "visitor");
        let room = application
            .create_room(
                owner.id(),
                CreateRoom {
                    name: "规则测试".to_owned(),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Yonma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        let room = application
            .join_room(guest.id(), room.id(), room.version())
            .expect("join");
        let room = application
            .set_ready(guest.id(), room.id(), room.version(), true)
            .expect("ready");
        let room = application
            .update_room(
                owner.id(),
                room.id(),
                UpdateRoom {
                    expected_version: room.version(),
                    name: None,
                    visibility: None,
                    rules: Some(RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Sanma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    }),
                },
            )
            .expect("update rules");

        assert!(room.members().iter().all(|member| !member.ready()));
        assert_eq!(room.rule_snapshot().seat_count(), 3);
    }

    #[test]
    fn owner_leaving_transfers_ownership_by_join_order() {
        let application = Application::new();
        let (owner, _) = register(&application, "first_host");
        let (first_guest, _) = register(&application, "first_guest");
        let (second_guest, _) = register(&application, "second_guest");
        let room = application
            .create_room(
                owner.id(),
                CreateRoom {
                    name: "房主转移".to_owned(),
                    visibility: RoomVisibility::Public,
                    rules: RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Yonma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        let room = application
            .join_room(first_guest.id(), room.id(), room.version())
            .expect("first join");
        let room = application
            .join_room(second_guest.id(), room.id(), room.version())
            .expect("second join");
        let room = application
            .leave_room(owner.id(), room.id(), room.version())
            .expect("owner leave");

        assert_eq!(room.owner_user_id(), first_guest.id());
    }

    #[test]
    fn closed_public_room_is_removed_from_the_joinable_lobby() {
        let application = Application::new();
        let (owner, _) = register(&application, "closed_room_host");
        let room = application
            .create_room(
                owner.id(),
                CreateRoom {
                    name: "即将关闭".to_owned(),
                    visibility: RoomVisibility::Public,
                    rules: RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Yonma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        assert_eq!(application.list_rooms().expect("list").len(), 1);

        application
            .leave_room(owner.id(), room.id(), room.version())
            .expect("close room");

        assert!(application.list_rooms().expect("list").is_empty());
        assert_eq!(
            application
                .room(room.id())
                .expect_err("empty room must be removed")
                .code(),
            ErrorCode::RoomNotFound
        );
    }

    #[test]
    fn start_requires_full_ready_room_and_creates_match_once() {
        let application = Application::new();
        let (owner, _) = register(&application, "match_host");
        let (guest_one, _) = register(&application, "match_guest_one");
        let (guest_two, _) = register(&application, "match_guest_two");
        let room = application
            .create_room(
                owner.id(),
                CreateRoom {
                    name: "三麻开局".to_owned(),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: mahjong_riichi::RiichiVariant::Sanma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        assert_eq!(
            application
                .start_room(owner.id(), room.id(), room.version(), 0)
                .expect_err("not full")
                .code(),
            ErrorCode::RoomNotReady
        );
        let room = application
            .join_room(guest_one.id(), room.id(), room.version())
            .expect("first join");
        let room = application
            .join_room(guest_two.id(), room.id(), room.version())
            .expect("second join");
        let room = application
            .set_ready(owner.id(), room.id(), room.version(), true)
            .expect("owner ready");
        let room = application
            .set_ready(guest_one.id(), room.id(), room.version(), true)
            .expect("first ready");
        let room = application
            .set_ready(guest_two.id(), room.id(), room.version(), true)
            .expect("second ready");
        let (started, match_id) = application
            .start_room(owner.id(), room.id(), room.version(), 0)
            .expect("start");

        assert_eq!(started.active_match_id(), Some(&match_id));
        assert_eq!(started.lifecycle(), crate::RoomLifecycle::Playing);
        assert_eq!(
            application
                .start_room(owner.id(), room.id(), started.version(), 0)
                .expect_err("already playing")
                .code(),
            ErrorCode::RoomPlaying
        );

        let players = [&owner, &guest_one, &guest_two];
        report_assets_ready(&application, &match_id, players);
        let mut view = application
            .match_view(owner.id(), &match_id)
            .expect("initial match view");
        for _ in 0..500 {
            if view.hand_index() > 0 {
                break;
            }
            if view.opening_ready_seats().count() < players.len() {
                // 开局摸牌动画播完之前谁也不许动手，各家先报告一声。
                for actor in players {
                    let actor_view = application
                        .match_view(actor.id(), &match_id)
                        .expect("opening view");
                    view = application
                        .submit_game_command(
                            actor.id(),
                            &match_id,
                            SubmitGameCommand {
                                expected_version: actor_view.version(),
                                command: GameCommand::ReadyForHand {
                                    hand_index: actor_view.hand_index(),
                                },
                            },
                            0,
                        )
                        .expect("ready for hand");
                }
                continue;
            }
            match view.phase() {
                mahjong_riichi::HandPhase::AwaitingTurnAction { seat }
                | mahjong_riichi::HandPhase::AwaitingDiscard { seat } => {
                    let seated_user_id =
                        view.players()[usize::from(seat.index())].player().user_id();
                    let actor = players
                        .iter()
                        .find(|player| player.id() == seated_user_id)
                        .expect("acting seat belongs to a registered player");
                    let actor_view = application
                        .match_view(actor.id(), &match_id)
                        .expect("actor view");
                    let tile_id = actor_view.players()[usize::from(seat.index())]
                        .concealed_tiles()
                        .expect("own concealed hand")[0]
                        .id()
                        .value();
                    view = application
                        .submit_game_command(
                            actor.id(),
                            &match_id,
                            SubmitGameCommand {
                                expected_version: actor_view.version(),
                                command: GameCommand::Discard { tile_id },
                            },
                            0,
                        )
                        .expect("discard");
                }
                mahjong_riichi::HandPhase::AwaitingResponses { .. } => {
                    for actor in players {
                        let actor_view = application
                            .match_view(actor.id(), &match_id)
                            .expect("responder view");
                        if actor_view.available_reactions().is_empty() {
                            continue;
                        }
                        view = application
                            .submit_game_command(
                                actor.id(),
                                &match_id,
                                SubmitGameCommand {
                                    expected_version: actor_view.version(),
                                    command: GameCommand::Pass,
                                },
                                0,
                            )
                            .expect("pass");
                    }
                }
                mahjong_riichi::HandPhase::Ended { .. } => {
                    // 先各家报告结算动画播完，服务端开了确认窗口才收得下确认。
                    for command in SETTLEMENT_HANDSHAKE {
                        for actor in players {
                            let actor_view = application
                                .match_view(actor.id(), &match_id)
                                .expect("settlement view");
                            view = application
                                .submit_game_command(
                                    actor.id(),
                                    &match_id,
                                    SubmitGameCommand {
                                        expected_version: actor_view.version(),
                                        command: command(actor_view.hand_index()),
                                    },
                                    0,
                                )
                                .expect("settlement handshake");
                        }
                    }
                }
            }
        }

        assert_eq!(view.hand_index(), 1);
        assert_eq!(view.progress().round_number().value(), 2);
        assert!(view.event_sequence() > 100);
    }

    #[test]
    fn yonma_and_sanma_can_each_finish_a_complete_east_only_match() {
        for variant in [RiichiVariant::Yonma, RiichiVariant::Sanma] {
            finish_east_only_match(variant);
        }
    }

    fn finish_east_only_match(variant: RiichiVariant) {
        let application = Application::new();
        let seat_count = usize::from(variant.seat_count().value());
        let prefix = if variant == RiichiVariant::Yonma {
            "yonma"
        } else {
            "sanma"
        };
        let players = (0..seat_count)
            .map(|index| register(&application, &format!("{prefix}_{index}")).0)
            .collect::<Vec<_>>();
        let room = application
            .create_room(
                players[0].id(),
                CreateRoom {
                    name: format!("{prefix} complete match"),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant,
                        request: mahjong_riichi::RoomRuleRequest {
                            preset: None,
                            overrides: RiichiRuleOverrides {
                                match_rules: Some(MatchRuleOverrides {
                                    length: Some(MatchLength::EastOnly),
                                    first_place_required_points: Some(25_000),
                                    tobi: Some(false),
                                    dealer_continuation: Some(DealerContinuation::WinOnly),
                                    agari_yame: Some(false),
                                    ..MatchRuleOverrides::default()
                                }),
                                scoring: Some(ScoringRuleOverrides {
                                    nagashi_mangan: Some(false),
                                    ..ScoringRuleOverrides::default()
                                }),
                                abortive_draws: Some(AbortiveDrawRuleOverrides {
                                    four_winds: Some(false),
                                    four_kans: Some(false),
                                    nine_terminals: Some(false),
                                    four_riichi: Some(false),
                                }),
                                ..RiichiRuleOverrides::default()
                            },
                        },
                    },
                },
            )
            .expect("room");
        let mut room = room;
        for player in &players[1..] {
            room = application
                .join_room(player.id(), room.id(), room.version())
                .expect("join");
        }
        for player in &players {
            room = application
                .set_ready(player.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = application
            .start_room(players[0].id(), room.id(), room.version(), 0)
            .expect("start");
        report_assets_ready(&application, &match_id, &players);
        let mut view = application
            .match_view(players[0].id(), &match_id)
            .expect("initial view");

        for _ in 0..5_000 {
            if view.result().is_some() {
                break;
            }
            if view.opening_ready_seats().count() < players.len() {
                // 每一局都要重来一遍：开局摸牌动画播完之前服务端不放行出牌。
                for actor in &players {
                    let actor_view = application
                        .match_view(actor.id(), &match_id)
                        .expect("opening view");
                    view = application
                        .submit_game_command(
                            actor.id(),
                            &match_id,
                            SubmitGameCommand {
                                expected_version: actor_view.version(),
                                command: GameCommand::ReadyForHand {
                                    hand_index: actor_view.hand_index(),
                                },
                            },
                            0,
                        )
                        .expect("ready for hand");
                }
                continue;
            }
            match view.phase() {
                mahjong_riichi::HandPhase::AwaitingTurnAction { seat }
                | mahjong_riichi::HandPhase::AwaitingDiscard { seat } => {
                    let seated_user_id =
                        view.players()[usize::from(seat.index())].player().user_id();
                    let actor = players
                        .iter()
                        .find(|player| player.id() == seated_user_id)
                        .expect("acting seat belongs to a registered player");
                    let actor_view = application
                        .match_view(actor.id(), &match_id)
                        .expect("actor view");
                    let tile_id = actor_view
                        .players()
                        .iter()
                        .find(|player| player.player().seat() == seat)
                        .and_then(crate::ObserverPlayer::concealed_tiles)
                        .expect("own concealed tiles")[0]
                        .id()
                        .value();
                    view = application
                        .submit_game_command(
                            actor.id(),
                            &match_id,
                            SubmitGameCommand {
                                expected_version: actor_view.version(),
                                command: GameCommand::Discard { tile_id },
                            },
                            0,
                        )
                        .expect("discard");
                }
                mahjong_riichi::HandPhase::AwaitingResponses { .. } => {
                    for actor in &players {
                        let actor_view = application
                            .match_view(actor.id(), &match_id)
                            .expect("responder view");
                        if actor_view.available_reactions().is_empty() {
                            continue;
                        }
                        view = application
                            .submit_game_command(
                                actor.id(),
                                &match_id,
                                SubmitGameCommand {
                                    expected_version: actor_view.version(),
                                    command: GameCommand::Pass,
                                },
                                0,
                            )
                            .expect("pass");
                    }
                }
                mahjong_riichi::HandPhase::Ended { .. } => {
                    // 结算是两段握手：各家先报告动画播完，服务端开了确认窗口
                    // 才收得下确认。顺序反过来会被拒，那五秒倒计时因此对全场
                    // 是同一个数。
                    let pending = application
                        .match_view(players[0].id(), &match_id)
                        .expect("settlement view");
                    assert!(
                        pending
                            .hand_settlement()
                            .expect("pending settlement")
                            .confirm_deadline_ms()
                            .is_none(),
                        "谁都还没报告播完，确认窗口不该开"
                    );
                    assert_eq!(
                        application
                            .submit_game_command(
                                players[0].id(),
                                &match_id,
                                SubmitGameCommand {
                                    expected_version: pending.version(),
                                    command: GameCommand::ConfirmSettlement {
                                        hand_index: pending.hand_index(),
                                    },
                                },
                                0,
                            )
                            .expect_err("the settlement is still being played")
                            .code(),
                        ErrorCode::InvalidGameCommand
                    );
                    for (step, command) in SETTLEMENT_HANDSHAKE.into_iter().enumerate() {
                        if step == 1 {
                            // 全场都报告完了，窗口这才开。
                            assert!(
                                application
                                    .match_view(players[0].id(), &match_id)
                                    .expect("settlement view")
                                    .hand_settlement()
                                    .expect("pending settlement")
                                    .confirm_deadline_ms()
                                    .is_some(),
                                "全场都播完了，确认窗口该开了"
                            );
                        }
                        for actor in &players {
                            let actor_view = application
                                .match_view(actor.id(), &match_id)
                                .expect("settlement view");
                            view = application
                                .submit_game_command(
                                    actor.id(),
                                    &match_id,
                                    SubmitGameCommand {
                                        expected_version: actor_view.version(),
                                        command: command(actor_view.hand_index()),
                                    },
                                    0,
                                )
                                .expect("settlement handshake");
                        }
                    }
                }
            }
        }

        let result = view
            .result()
            .expect("match must finish within command bound");
        assert_eq!(
            result.hand_count(),
            u32::try_from(seat_count).expect("seat count")
        );
        assert_eq!(
            result.final_points().iter().sum::<i32>(),
            i32::try_from(seat_count).expect("seat count") * 25_000
        );
        let record = application
            .match_record(players[0].id(), &match_id)
            .expect("match record");
        assert_eq!(record.hand_count(), seat_count);
        assert!(record.is_finished());
        let encoded = serde_json::to_value(record).expect("serialize record");
        assert_eq!(encoded["schema"], "match_record.v1");
        assert_eq!(
            encoded["hands"].as_array().expect("hand records").len(),
            seat_count
        );
        assert!(
            encoded["hands"][0]["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );
        assert_eq!(
            encoded["hands"][0]["events"][0]["name"],
            "riichi.hand_started"
        );
        assert!(encoded["result"]["placements"].is_array());
        assert!(encoded["rule_snapshot"]["config"].is_object());
        assert!(encoded["friend_match"].is_boolean());

        // 对局打完了，牌山才随牌谱出来：每一局都得有一份，长度是整副牌，而且事件
        // 日志里摸到的每一张牌都能在里面找到——重演的三色牌山全靠这个对应关系。
        for hand in encoded["hands"].as_array().expect("hand records") {
            let wall = &hand["wall"];
            let tiles = wall["tiles"]
                .as_array()
                .expect("finished hands carry a wall");
            assert!(!tiles.is_empty());
            let live_end = wall["live_end"].as_u64().expect("live wall end");
            assert_eq!(
                usize::try_from(live_end).expect("live end fits") + 14,
                tiles.len(),
                "王牌固定十四张"
            );
            let wall_ids: std::collections::HashSet<_> = tiles
                .iter()
                .map(|tile| tile["id"].as_u64().expect("wall tile id"))
                .collect();
            for event in hand["events"].as_array().expect("hand events") {
                if event["name"] == "riichi.tile_drawn" {
                    let drawn = event["payload"]["tile"]["id"]
                        .as_u64()
                        .expect("drawn tile id");
                    assert!(wall_ids.contains(&drawn), "摸到的牌必须来自本局牌山");
                }
            }

            // 和牌那几家的番符明细一家一条，流局就一条都没有；里宝牌同理——流局不翻。
            let winners = hand["winners"].as_array().expect("winner seats");
            let scores = hand["winner_scores"].as_array().expect("winner scores");
            assert_eq!(scores.len(), winners.len());
            for (score, seat) in scores.iter().zip(winners) {
                assert_eq!(&score["seat"], seat);
                assert!(score["yaku"].is_array(), "和牌必须写出役种");
                assert!(score["han"].as_u64().is_some_and(|han| han > 0));
            }
            let ura = hand["ura_dora_indicators"]
                .as_array()
                .expect("ura dora indicators");
            if winners.is_empty() {
                assert!(ura.is_empty(), "流局不翻里宝牌");
            }
        }
    }

    /// 牌山是这局唯一藏得住的东西：还在打的时候把它发出去，等于给客户端一份作弊器。
    #[test]
    fn an_unfinished_match_record_never_carries_the_wall() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "wall_gate");

        let record = application
            .match_record(players[0].id(), &match_id)
            .expect("match record");
        assert!(!record.is_finished());
        let encoded = serde_json::to_value(record).expect("serialize record");
        for hand in encoded["hands"].as_array().expect("hand records") {
            assert!(hand["wall"].is_null(), "对局没结束就不许下发牌山");
        }
    }

    /// Starts a sanma table at `now_ms = 0`, returning the players and match.
    fn started_sanma_table_unready(
        application: &Application,
        prefix: &str,
    ) -> (Vec<crate::User>, mahjong_core::MatchId) {
        let players = (0..3)
            .map(|index| register(application, &format!("{prefix}_{index}")).0)
            .collect::<Vec<_>>();
        let mut room = application
            .create_room(
                players[0].id(),
                CreateRoom {
                    name: format!("{prefix} clock table"),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: RiichiVariant::Sanma,
                        request: mahjong_riichi::RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        for player in &players[1..] {
            room = application
                .join_room(player.id(), room.id(), room.version())
                .expect("join");
        }
        for player in &players {
            room = application
                .set_ready(player.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = application
            .start_room(players[0].id(), room.id(), room.version(), 0)
            .expect("start");
        // 素材load是开局第一道门，不过它不属于「开局动画握手」，这里一律先过掉。
        report_assets_ready(application, &match_id, &players);
        (players, match_id)
    }

    /// 每家都报告对局素材load完了。这是开局第一道门，不过它一步都走不了。
    fn report_assets_ready<'a>(
        application: &Application,
        match_id: &mahjong_core::MatchId,
        players: impl IntoIterator<Item = &'a crate::User>,
    ) {
        for player in players {
            application
                .submit_game_command(
                    player.id(),
                    match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::MatchAssetsReady,
                    },
                    0,
                )
                .expect("assets ready");
        }
    }

    fn started_sanma_table(
        application: &Application,
        prefix: &str,
    ) -> (Vec<crate::User>, mahjong_core::MatchId) {
        let (players, match_id) = started_sanma_table_unready(application, prefix);
        for player in &players {
            application
                .submit_game_command(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::ReadyForHand { hand_index: 0 },
                    },
                    0,
                )
                .expect("opening ready");
        }
        (players, match_id)
    }

    /// 一路摸切把当前这局打到结算挂起为止，不关心牌效。
    fn play_until_settlement(
        application: &Application,
        players: &[crate::User],
        match_id: &mahjong_core::MatchId,
    ) {
        for _ in 0..5_000 {
            let view = application
                .match_view(players[0].id(), match_id)
                .expect("view");
            if view.hand_settlement().is_some() {
                return;
            }
            match view.phase() {
                mahjong_riichi::HandPhase::AwaitingTurnAction { seat }
                | mahjong_riichi::HandPhase::AwaitingDiscard { seat } => {
                    let seated_user_id =
                        view.players()[usize::from(seat.index())].player().user_id();
                    let actor = players
                        .iter()
                        .find(|player| player.id() == seated_user_id)
                        .expect("acting seat belongs to a registered player");
                    let actor_view = application
                        .match_view(actor.id(), match_id)
                        .expect("actor view");
                    let tile_id = actor_view
                        .players()
                        .iter()
                        .find(|player| player.player().seat() == seat)
                        .and_then(crate::ObserverPlayer::concealed_tiles)
                        .expect("own concealed tiles")[0]
                        .id()
                        .value();
                    application
                        .submit_game_command(
                            actor.id(),
                            match_id,
                            SubmitGameCommand {
                                expected_version: actor_view.version(),
                                command: GameCommand::Discard { tile_id },
                            },
                            0,
                        )
                        .expect("discard");
                }
                mahjong_riichi::HandPhase::AwaitingResponses { .. } => {
                    for actor in players {
                        let actor_view = application
                            .match_view(actor.id(), match_id)
                            .expect("responder view");
                        if actor_view.available_reactions().is_empty() {
                            continue;
                        }
                        application
                            .submit_game_command(
                                actor.id(),
                                match_id,
                                SubmitGameCommand {
                                    expected_version: actor_view.version(),
                                    command: GameCommand::Pass,
                                },
                                0,
                            )
                            .expect("pass");
                    }
                }
                mahjong_riichi::HandPhase::Ended { .. } => return,
            }
        }
        panic!("the hand never reached a settlement");
    }

    /// 开一张四人冲击麻将桌，过掉素材 load 与开局动画两道门。
    fn started_impact_table(
        application: &Application,
        prefix: &str,
    ) -> (Vec<crate::User>, mahjong_core::MatchId) {
        let players = (0..4)
            .map(|index| register(application, &format!("{prefix}_{index}")).0)
            .collect::<Vec<_>>();
        let mut room = application
            .create_room(
                players[0].id(),
                CreateRoom {
                    name: format!("{prefix} impact table"),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Impact {
                        request: mahjong_impact::ImpactRoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        for player in &players[1..] {
            room = application
                .join_room(player.id(), room.id(), room.version())
                .expect("join");
        }
        for player in &players {
            room = application
                .set_ready(player.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = application
            .start_room(players[0].id(), room.id(), room.version(), 0)
            .expect("start");
        // 冲击麻将没有立直那套牌谱投影，命令只能走通用的 submit_game。
        for player in &players {
            application
                .submit_game(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::MatchAssetsReady,
                    },
                    0,
                )
                .expect("assets ready");
        }
        for player in &players {
            application
                .submit_game(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::ReadyForHand { hand_index: 0 },
                    },
                    0,
                )
                .expect("opening ready");
        }
        (players, match_id)
    }

    /// 四家都报告杠点动画播完后，服务端才摸岭上牌。
    #[test]
    fn all_four_seats_acking_the_kan_animation_draws_the_rinshan_tile() {
        let application = Application::new();
        let (players, match_id) = started_impact_table(&application, "impact_kan");

        let projection = application
            .match_projection(players[0].id(), &match_id)
            .expect("projection");
        let view = projection.as_impact().expect("impact view").clone();
        let dealer_seat = view.dealer;
        let dealer_user = projection
            .seated()
            .into_iter()
            .find(|(seat, _)| *seat == dealer_seat)
            .map(|(_, user)| user.clone())
            .expect("dealer seat");
        let dealer = players
            .iter()
            .find(|player| player.id() == &dealer_user)
            .expect("dealer player");

        // 挑一张既不是财神、也不是指示牌的字牌来暗杠，避开财神/指示牌的岔路。
        let candidates = ["1z", "2z", "3z", "4z", "5z", "6z", "7z"]
            .into_iter()
            .map(str::to_owned)
            .filter(|code| {
                Some(code) != view.joker_code.as_ref()
                    && Some(code) != view.joker_indicator.as_ref().map(|tile| &tile.code)
            })
            .collect::<Vec<_>>();
        let kan_code = candidates[0].clone();
        let mut hand_codes = vec![kan_code.clone(); 4];
        hand_codes.extend(std::iter::repeat_n(candidates[1].clone(), 4));
        hand_codes.extend(std::iter::repeat_n(candidates[2].clone(), 3));
        hand_codes.extend(std::iter::repeat_n(candidates[3].clone(), 3));
        application
            .set_dev_hand(dealer.id(), &match_id, &hand_codes)
            .expect("dev hand");

        let dealer_view = application
            .match_projection(dealer.id(), &match_id)
            .expect("dealer view");
        let version = dealer_view.version();
        assert_eq!(
            dealer_view.as_impact().expect("impact").completed_rinshan_draws,
            0,
            "杠之前还没有岭上补摸"
        );

        let after_kan = application
            .submit_game(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: version,
                    command: GameCommand::ImpactConcealedKan {
                        tile_code: kan_code,
                    },
                },
                0,
            )
            .expect("concealed kan");
        let kan_view = after_kan.as_impact().expect("impact").clone();
        // 杠完先等四家播杠点动画，还没摸岭上牌。
        assert_eq!(kan_view.phase_kind, "awaiting_kan_animation");
        assert_eq!(kan_view.completed_rinshan_draws, 0);
        let kan_id = kan_view.last_kan.expect("last kan").id;

        // 前三家报告播完，还差一家，岭上牌仍不摸。
        for player in &players[..3] {
            let v = application
                .match_projection(player.id(), &match_id)
                .expect("ack view")
                .version();
            application
                .submit_game(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: v,
                        command: GameCommand::ImpactKanAnimationPlayed { kan_id },
                    },
                    0,
                )
                .expect("ack");
        }
        let after_three = application
            .match_projection(players[0].id(), &match_id)
            .expect("after three acks");
        let after_three = after_three.as_impact().expect("impact");
        assert_eq!(after_three.phase_kind, "awaiting_kan_animation");
        assert_eq!(after_three.completed_rinshan_draws, 0);

        // 最后一家报告播完，服务端才摸岭上牌。
        let last = &players[3];
        let v = application
            .match_projection(last.id(), &match_id)
            .expect("last ack view")
            .version();
        let after_all = application
            .submit_game(
                last.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: v,
                    command: GameCommand::ImpactKanAnimationPlayed { kan_id },
                },
                0,
            )
            .expect("final ack");
        let after_all = after_all.as_impact().expect("impact");
        assert_eq!(after_all.phase_kind, "awaiting_turn_action");
        assert_eq!(after_all.completed_rinshan_draws, 1);

        // 岭上牌只在庄家自己的投影里露脸（对手视角应当藏起来，否则等于摊牌）。
        let dealer_projection = application
            .match_projection(dealer.id(), &match_id)
            .expect("dealer projection");
        let dealer_view = dealer_projection.as_impact().expect("impact");
        let drawn_player = dealer_view
            .players
            .iter()
            .find(|player| player.player.seat() == dealer_seat)
            .expect("dealer player view");
        assert!(
            drawn_player.drawn_tile_id.is_some(),
            "庄家摸到一张新的岭上牌"
        );
    }

    /// 四家都没报告、杠点动画兜底超时后，服务端仍要广播摸岭上牌，不能只改状态不广播。
    #[test]
    fn a_kan_animation_timeout_still_broadcasts_the_rinshan_draw() {
        let application = Application::new();
        let (players, match_id) = started_impact_table(&application, "impact_kan_timeout");

        let projection = application
            .match_projection(players[0].id(), &match_id)
            .expect("projection");
        let view = projection.as_impact().expect("impact view").clone();
        let dealer_seat = view.dealer;
        let dealer_user = projection
            .seated()
            .into_iter()
            .find(|(seat, _)| *seat == dealer_seat)
            .map(|(_, user)| user.clone())
            .expect("dealer seat");
        let dealer = players
            .iter()
            .find(|player| player.id() == &dealer_user)
            .expect("dealer player");

        let candidates = ["1z", "2z", "3z", "4z", "5z", "6z", "7z"]
            .into_iter()
            .map(str::to_owned)
            .filter(|code| {
                Some(code) != view.joker_code.as_ref()
                    && Some(code) != view.joker_indicator.as_ref().map(|tile| &tile.code)
            })
            .collect::<Vec<_>>();
        let kan_code = candidates[0].clone();
        let mut hand_codes = vec![kan_code.clone(); 4];
        hand_codes.extend(std::iter::repeat_n(candidates[1].clone(), 4));
        hand_codes.extend(std::iter::repeat_n(candidates[2].clone(), 3));
        hand_codes.extend(std::iter::repeat_n(candidates[3].clone(), 3));
        application
            .set_dev_hand(dealer.id(), &match_id, &hand_codes)
            .expect("dev hand");

        let dealer_view = application
            .match_projection(dealer.id(), &match_id)
            .expect("dealer view");
        let version = dealer_view.version();
        let after_kan = application
            .submit_game(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: version,
                    command: GameCommand::ImpactConcealedKan {
                        tile_code: kan_code,
                    },
                },
                0,
            )
            .expect("concealed kan");
        let kan_view = after_kan.as_impact().expect("impact").clone();
        assert_eq!(kan_view.phase_kind, "awaiting_kan_animation");
        assert_eq!(kan_view.completed_rinshan_draws, 0);

        // 谁都没报告，直接越过 6 秒兜底：扫描必须返回一次广播，否则前端永远看不到岭上牌。
        let expiries = application
            .expire_clocks(6_000)
            .expect("sweep past the kan animation fallback");
        assert_eq!(
            expiries.len(),
            1,
            "兜底超时也要广播一次，通知全体摸岭上牌"
        );

        let after_timeout = application
            .match_projection(players[0].id(), &match_id)
            .expect("after timeout view");
        let after_timeout = after_timeout.as_impact().expect("impact");
        assert_eq!(after_timeout.phase_kind, "awaiting_turn_action");
        assert_eq!(after_timeout.completed_rinshan_draws, 1);
    }

    #[test]
    fn the_settlement_confirm_window_opens_for_everybody_at_once() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "settle_window");
        play_until_settlement(&application, &players, &match_id);

        let settlement_of = |seat_index: usize| {
            let view = application
                .match_view(players[seat_index].id(), &match_id)
                .expect("settlement view");
            let settlement = view.hand_settlement().expect("pending settlement");
            (
                settlement.played_seats().len(),
                settlement.confirm_deadline_ms(),
                view.version(),
            )
        };

        // 谁都还没播完，确认窗口没开，这时候点确认要被拒。
        let (played, deadline_ms, version) = settlement_of(0);
        assert_eq!(played, 0);
        assert_eq!(deadline_ms, None);
        assert_eq!(
            application
                .submit_game_command(
                    players[0].id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: version,
                        command: GameCommand::ConfirmSettlement { hand_index: 0 },
                    },
                    0,
                )
                .expect_err("the settlement is still being played")
                .code(),
            ErrorCode::InvalidGameCommand
        );

        // 第一家播完了，但还得等其他家，窗口仍然不开。
        application
            .submit_game_command(
                players[0].id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: version,
                    command: GameCommand::SettlementPlayed { hand_index: 0 },
                },
                1_000,
            )
            .expect("first settlement played");
        assert_eq!(settlement_of(0).1, None);

        // 全场都没报告完，等兜底到期后服务端替所有人补上并开窗，倒计时对全场同时起算。
        // 结算挂起于时间 0，兜底截止按役种条数动态计算。
        let yaku_count: usize = {
            let view = application
                .match_view(players[0].id(), &match_id)
                .expect("view for yaku count");
            view.hand_settlement()
                .expect("pending settlement")
                .winners()
                .iter()
                .map(|w| w.evaluation().yaku().len())
                .sum()
        };
        let fallback_ms = crate::settlement_reveal_fallback_ms(yaku_count);
        application
            .expire_clocks(fallback_ms)
            .expect("open the confirm window");
        for seat_index in 0..players.len() {
            let (played, deadline_ms, _) = settlement_of(seat_index);
            assert_eq!(played, players.len(), "开窗时当作全场都播完了");
            assert!(deadline_ms.is_some(), "各家都看到了确认截止时刻");
        }

        // 倒计时走完，谁都不点也照开下一局。
        application
            .expire_clocks(fallback_ms + crate::SETTLEMENT_CONFIRM_MS)
            .expect("advance the settlement");
        let next = application
            .match_view(players[0].id(), &match_id)
            .expect("next hand view");
        assert!(next.hand_settlement().is_none());
        assert_eq!(next.hand_index(), 1);
    }

    #[test]
    fn exit_vote_defaults_missing_votes_to_agree_after_fifteen_seconds() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "exit_default");
        let before = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let room_id = before.room_id().clone();

        let voting = application
            .submit_game_command(
                players[0].id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: before.version(),
                    command: GameCommand::RequestExitVote,
                },
                1_000,
            )
            .expect("start exit vote");

        assert!(voting.exit_vote().is_some());
        assert!(
            voting
                .clocks()
                .iter()
                .all(|clock| clock.armed_at_ms().is_none())
        );
        assert!(voting.available_reactions().is_empty());

        let expiries = application
            .expire_clocks(1_000 + 15_000)
            .expect("expire vote");
        assert_eq!(expiries.len(), 1);
        assert!(expiries[0].finished);

        let finished = application
            .match_view(players[0].id(), &match_id)
            .expect("finished view");
        assert!(finished.terminated_by_exit_vote());
        assert!(finished.exit_vote().is_none());
        let room = application.room(&room_id).expect("room");
        assert_eq!(room.lifecycle(), crate::RoomLifecycle::Waiting);
        assert_eq!(room.active_match_id(), None);
    }

    #[test]
    fn a_rejected_exit_vote_resumes_clocks_and_cannot_be_restarted_by_initiator() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "exit_vote_rejected");
        let before = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let deadlines = before
            .clocks()
            .iter()
            .map(crate::SeatClock::deadline_ms)
            .collect::<Vec<_>>();

        let mut view = application
            .submit_game_command(
                players[0].id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: before.version(),
                    command: GameCommand::RequestExitVote,
                },
                1_000,
            )
            .expect("start exit vote");
        for (offset, player) in players[1..].iter().enumerate() {
            view = application
                .submit_game_command(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: view.version(),
                        command: GameCommand::VoteExit { agree: false },
                    },
                    2_000 + u64::try_from(offset).expect("offset") * 1_000,
                )
                .expect("reject exit vote");
        }

        assert!(view.exit_vote().is_none());
        assert!(!view.terminated_by_exit_vote());
        assert_eq!(
            view.clocks()
                .iter()
                .map(crate::SeatClock::deadline_ms)
                .collect::<Vec<_>>(),
            deadlines
                .into_iter()
                .map(|deadline| deadline.map(|value| value + 2_000))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            application
                .submit_game_command(
                    players[0].id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: view.version(),
                        command: GameCommand::RequestExitVote,
                    },
                    4_000,
                )
                .expect_err("initiator used this hand's vote")
                .code(),
            ErrorCode::InvalidGameCommand
        );
    }

    /// The moment at which a seat armed at `armed_at_ms` runs out of time.
    fn full_thinking_time(armed_at_ms: u64) -> u64 {
        armed_at_ms + crate::BASE_THINKING_MS + u64::from(crate::RESERVE_THINKING_MS)
    }

    #[test]
    fn first_clock_waits_until_every_player_finishes_the_opening() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table_unready(&application, "clock_opening_ready");

        for player in &players[..2] {
            let view = application
                .submit_game_command(
                    player.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::ReadyForHand { hand_index: 0 },
                    },
                    0,
                )
                .expect("partial opening ready");
            assert!(
                view.clocks()
                    .iter()
                    .all(|clock| clock.armed_at_ms().is_none())
            );
        }

        let ready = application
            .submit_game_command(
                players[2].id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: 0,
                    command: GameCommand::ReadyForHand { hand_index: 0 },
                },
                0,
            )
            .expect("all opening ready");
        assert_eq!(ready.opening_ready_seats().count(), 3);
        assert_eq!(
            ready
                .clocks()
                .iter()
                .filter(|clock| clock.armed_at_ms().is_some())
                .count(),
            1
        );
    }

    #[test]
    fn nobody_may_act_before_every_client_has_played_the_opening_deal() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table_unready(&application, "opening_gate");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = view.phase() else {
            panic!("a fresh hand waits for the dealer");
        };
        let dealer_user_id = view.players()[usize::from(seat.index())].player().user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("the dealer is one of the players");
        let dealer_view = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view");
        let tile_id = dealer_view.players()[usize::from(seat.index())]
            .concealed_tiles()
            .expect("own concealed hand")[0]
            .id()
            .value();
        let discard = |now_ms: u64| {
            let dealer_view = application
                .match_view(dealer.id(), &match_id)
                .expect("dealer view");
            application.submit_game_command(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: dealer_view.version(),
                    command: GameCommand::Discard { tile_id },
                },
                now_ms,
            )
        };

        // 牌还在往各家手上飞，庄家的客户端就算抢跑也打不出去。
        assert_eq!(
            discard(0)
                .expect_err("the opening deal is still running")
                .code(),
            ErrorCode::InvalidGameCommand
        );

        application
            .submit_game_command(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: 0,
                    command: GameCommand::ReadyForHand { hand_index: 0 },
                },
                1_000,
            )
            .expect("first opening ready");
        // 自己报告完也不行，得等其他家。
        assert_eq!(
            discard(1_000)
                .expect_err("the other seats are still dealing")
                .code(),
            ErrorCode::InvalidGameCommand
        );

        // 但只等一段固定的宽限，掉线或者被浏览器节流的人不会卡住整桌。
        let released_at_ms = 1_000 + crate::ANIMATION_REPORT_GRACE_MS;
        application
            .expire_clocks(released_at_ms)
            .expect("release the opening gate");
        assert_eq!(
            application
                .match_view(dealer.id(), &match_id)
                .expect("released view")
                .opening_ready_seats()
                .count(),
            3
        );
        discard(released_at_ms).expect("the gate is open");
    }

    #[test]
    fn dealing_puts_only_the_dealer_on_the_clock() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_start");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");

        let armed: Vec<_> = view
            .clocks()
            .iter()
            .enumerate()
            .filter(|(_, clock)| clock.armed_at_ms().is_some())
            .map(|(index, _)| index)
            .collect();
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = view.phase() else {
            panic!("a fresh hand waits for the dealer");
        };
        assert_eq!(armed, vec![usize::from(seat.index())]);
        assert_eq!(
            view.clocks()[usize::from(seat.index())].deadline_ms(),
            Some(full_thinking_time(0))
        );
    }

    #[test]
    fn a_timeout_discards_the_drawn_tile() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_tsumogiri");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = application
            .match_view(players[0].id(), &match_id)
            .expect("view")
            .phase()
        else {
            panic!("a fresh hand waits for the dealer");
        };
        let public_view = application
            .match_view(players[0].id(), &match_id)
            .expect("public match view");
        let dealer_user_id = public_view.players()[usize::from(seat.index())]
            .player()
            .user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("dealer seat belongs to a registered player");
        let drawn = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view")
            .players()[usize::from(seat.index())]
        .drawn_tile_id()
        .expect("the dealer holds a drawn tile");

        let expiries = application
            .expire_clocks(full_thinking_time(0))
            .expect("sweep");

        assert_eq!(expiries.len(), 1);
        assert_eq!(expiries[0].match_id, match_id);
        assert!(!expiries[0].finished);
        let view = application
            .match_view(dealer.id(), &match_id)
            .expect("view after timeout");
        let discard = view.players()[usize::from(seat.index())]
            .discards()
            .last()
            .expect("the timeout discarded a tile");
        assert_eq!(discard.tile().id(), drawn);
        assert!(discard.is_tsumogiri());
        assert_eq!(view.clocks()[usize::from(seat.index())].reserve_ms(), 0);
    }

    #[test]
    fn a_timeout_declares_tsumo_when_the_drawn_tile_completes_a_win() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_tsumo");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = application
            .match_view(players[0].id(), &match_id)
            .expect("view")
            .phase()
        else {
            panic!("a fresh hand waits for the dealer");
        };
        let public_view = application
            .match_view(players[0].id(), &match_id)
            .expect("public match view");
        let dealer_user_id = public_view.players()[usize::from(seat.index())]
            .player()
            .user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("dealer seat belongs to a registered player");

        // 三麻没有二到八万，凑一副断幺九的听牌：任意一张都是完整牌型的一员，
        // 摸上来的那张正好凑齐四副顺子加一对，超时后应该自摸而不是切牌。
        let hand_codes = [
            "2p", "3p", "4p", "2s", "3s", "4s", "3p", "4p", "5p", "3s", "4s", "5s", "6s", "6s",
        ]
        .map(str::to_owned);
        application
            .set_dev_hand(dealer.id(), &match_id, &hand_codes)
            .expect("dev hand");

        let expiries = application
            .expire_clocks(full_thinking_time(0))
            .expect("sweep");

        assert_eq!(expiries.len(), 1);
        assert_eq!(expiries[0].match_id, match_id);
        let view = application
            .match_view(dealer.id(), &match_id)
            .expect("view after timeout");
        let settlement = view
            .hand_settlement()
            .expect("the timeout declared a tsumo");
        assert_eq!(settlement.winners().len(), 1);
        assert_eq!(settlement.winners()[0].seat(), seat);
    }

    #[test]
    fn a_timeout_declares_ron_when_a_discard_completes_a_win() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_ron");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = view.phase() else {
            panic!("a fresh hand waits for the dealer");
        };
        let dealer_user_id = view.players()[usize::from(seat.index())].player().user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("dealer seat belongs to a registered player");
        let responder_seat = mahjong_riichi::Seat::new(
            mahjong_riichi::RiichiVariant::Sanma,
            (seat.index() + 1) % 3,
        )
        .expect("the next seat exists");
        let responder_user_id = view.players()[usize::from(responder_seat.index())]
            .player()
            .user_id();
        let responder = players
            .iter()
            .find(|player| player.id() == responder_user_id)
            .expect("responder seat belongs to a registered player");

        // 让庄家摸上来的那张是 2s，弃牌必是 2s。
        let dealer_view = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view");
        let dealer_player = &dealer_view.players()[usize::from(seat.index())];
        let drawn_id = dealer_player
            .drawn_tile_id()
            .expect("dealer holds a drawn tile");
        let drawn_index = dealer_player
            .concealed_tiles()
            .expect("own concealed hand")
            .iter()
            .position(|tile| tile.id() == drawn_id)
            .expect("drawn tile is in hand");
        let mut dealer_codes = vec!["1p".to_owned(); 14];
        dealer_codes[drawn_index] = "2s".to_owned();
        application
            .set_dev_hand(dealer.id(), &match_id, &dealer_codes)
            .expect("dealer dev hand");

        // 下一家单骑听 2s（断幺九），荣到 2s 就能和。
        let responder_codes = [
            "2p", "3p", "4p", "4p", "5p", "6p", "2p", "3p", "4p", "4p", "5p", "6p", "2s",
        ]
        .map(str::to_owned);
        application
            .set_dev_hand(responder.id(), &match_id, &responder_codes)
            .expect("responder dev hand");

        // 庄家弃掉 2s，下家进入可以荣和的等待。
        let dealer_view = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view after dev hand");
        let discard_tile_id = dealer_view.players()[usize::from(seat.index())]
            .drawn_tile_id()
            .expect("drawn tile")
            .value();
        let discarded_at_ms = 1_000;
        application
            .submit_game_command(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: dealer_view.version(),
                    command: GameCommand::Discard {
                        tile_id: discard_tile_id,
                    },
                },
                discarded_at_ms,
            )
            .expect("discard");

        // 下家能荣 2s，超时后应该自动荣和，而不是过牌。
        let grace_ms = crate::discard_animation_ms();
        let expiries = application
            .expire_clocks(discarded_at_ms + grace_ms + full_thinking_time(0))
            .expect("sweep");

        assert_eq!(expiries.len(), 1);
        assert_eq!(expiries[0].match_id, match_id);
        let view = application
            .match_view(responder.id(), &match_id)
            .expect("view after timeout");
        let settlement = view.hand_settlement().expect("the timeout declared a ron");
        assert_eq!(settlement.winners().len(), 1);
        assert_eq!(settlement.winners()[0].seat(), responder_seat);
    }

    #[test]
    fn deciding_in_time_keeps_the_reserve_intact() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_reserve");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = view.phase() else {
            panic!("a fresh hand waits for the dealer");
        };
        let dealer_user_id = view.players()[usize::from(seat.index())].player().user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("dealer seat belongs to a registered player");
        let dealer_view = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view");
        let tile_id = dealer_view.players()[usize::from(seat.index())]
            .concealed_tiles()
            .expect("own concealed hand")[0]
            .id()
            .value();

        let view = application
            .submit_game_command(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: dealer_view.version(),
                    command: GameCommand::Discard { tile_id },
                },
                crate::BASE_THINKING_MS - 1,
            )
            .expect("discard");

        assert_eq!(
            view.clocks()[usize::from(seat.index())].reserve_ms(),
            crate::RESERVE_THINKING_MS
        );
        assert_eq!(view.clocks()[usize::from(seat.index())].armed_at_ms(), None);
    }

    #[test]
    fn the_next_seat_starts_counting_only_after_the_discard_lands() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_grace");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        let mahjong_riichi::HandPhase::AwaitingTurnAction { seat } = view.phase() else {
            panic!("a fresh hand waits for the dealer");
        };
        let dealer_user_id = view.players()[usize::from(seat.index())].player().user_id();
        let dealer = players
            .iter()
            .find(|player| player.id() == dealer_user_id)
            .expect("dealer seat belongs to a registered player");
        let dealer_view = application
            .match_view(dealer.id(), &match_id)
            .expect("dealer view");
        let tile_id = dealer_view.players()[usize::from(seat.index())]
            .concealed_tiles()
            .expect("own concealed hand")[0]
            .id()
            .value();

        let discarded_at_ms = 1_000;
        let view = application
            .submit_game_command(
                dealer.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: dealer_view.version(),
                    command: GameCommand::Discard { tile_id },
                },
                discarded_at_ms,
            )
            .expect("discard");

        // 牌还在飞向牌河，这段时间不算任何人的思考时间。
        let grace_ms = crate::discard_animation_ms();
        assert!(grace_ms > 0);
        let armed: Vec<_> = view
            .clocks()
            .iter()
            .filter_map(|clock| clock.armed_at_ms())
            .collect();
        assert!(!armed.is_empty(), "someone owes the next decision");
        for armed_at_ms in armed {
            assert_eq!(armed_at_ms, discarded_at_ms + grace_ms);
        }
        // 动画播完之前谁都不会超时，思考时间一秒不少。
        for clock in view.clocks() {
            let Some(deadline) = clock.deadline_ms() else {
                continue;
            };
            assert_eq!(deadline, full_thinking_time(discarded_at_ms + grace_ms));
        }
        assert!(
            application
                .expire_clocks(discarded_at_ms + grace_ms)
                .expect("sweep during the grace window")
                .is_empty()
        );
    }

    #[test]
    fn a_timeout_grants_the_same_animation_grace_as_a_played_tile() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_grace_timeout");
        let timed_out_at_ms = full_thinking_time(0);

        application
            .expire_clocks(timed_out_at_ms)
            .expect("sweep")
            .first()
            .expect("the dealer ran out of time");

        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view after timeout");
        let armed: Vec<_> = view
            .clocks()
            .iter()
            .filter_map(|clock| clock.armed_at_ms())
            .collect();
        assert!(!armed.is_empty(), "someone owes the next decision");
        for armed_at_ms in armed {
            assert_eq!(armed_at_ms, timed_out_at_ms + crate::discard_animation_ms());
        }
    }

    #[test]
    fn a_sweep_advances_a_match_at_most_once() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_once");
        let before = application
            .match_view(players[0].id(), &match_id)
            .expect("view")
            .version();

        let expiries = application
            .expire_clocks(full_thinking_time(0) * 10)
            .expect("sweep");

        assert_eq!(expiries.len(), 1);
        assert_eq!(
            application
                .match_view(players[0].id(), &match_id)
                .expect("view")
                .version(),
            before + 1
        );
    }

    #[test]
    fn a_table_where_nobody_acts_still_finishes() {
        let application = Application::new();
        let (players, match_id) = started_sanma_table(&application, "clock_afk");

        let mut now_ms = 0;
        let mut finished = false;
        for _ in 0..20_000 {
            now_ms += crate::BASE_THINKING_MS + u64::from(crate::RESERVE_THINKING_MS);
            for expiry in application.expire_clocks(now_ms).expect("sweep") {
                finished |= expiry.finished;
            }
            if finished {
                break;
            }
        }

        assert!(finished, "an idle table must reach its own conclusion");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("view");
        assert!(view.result().is_some());
        assert!(
            view.clocks()
                .iter()
                .all(|clock| clock.armed_at_ms().is_none())
        );
    }
}
