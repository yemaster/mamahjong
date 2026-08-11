use std::sync::Mutex;
use std::thread;

use mahjong_core::{SessionId, UserId};
use postgres::{Client, NoTls};

use crate::store::MemoryStore;
use crate::{
    AccountRole, AccountStatus, ApplicationError, Character, CharacterAsset, CharacterOutfit,
    CharacterSummary, CharacterVoice, ErrorCode, MusicScene, MusicTrack, Nickname, SaveCharacter,
    SaveMusicTrack, SaveTablecloth, Session, Tablecloth, User,
};

const IDENTITY_SCHEMA: &str = include_str!("../migrations/0001_identity.sql");
const CHARACTER_SCHEMA: &str = include_str!("../migrations/0002_characters.sql");
const USER_PRESENTATION_SCHEMA: &str = include_str!("../migrations/0003_user_presentation.sql");
const USER_OUTFIT_SCHEMA: &str = include_str!("../migrations/0004_user_outfit.sql");
const TABLECLOTH_SCHEMA: &str = include_str!("../migrations/0005_tablecloths.sql");
const MUSIC_SCHEMA: &str = include_str!("../migrations/0006_music_tracks.sql");
const RIICHI_MUSIC_SCHEMA: &str = include_str!("../migrations/0007_riichi_music.sql");
const MUSIC_SEED_SCHEMA: &str = include_str!("../migrations/0008_seed_music_tracks.sql");
type StoredUserRow = (
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type StoredSessionRow = (String, String, String);
type StoredCharacterRow = (
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    bool,
    bool,
);
type StoredTableclothRow = (String, i64, String, String, bool, bool);
type StoredMusicRow = (String, i64, String, String, String, i64, bool, bool);

pub(crate) struct PostgresIdentityStore {
    client: Mutex<Option<Client>>,
}

impl PostgresIdentityStore {
    pub(crate) fn connect(database_url: &str) -> Result<Self, ApplicationError> {
        let database_url = database_url.to_owned();
        let client = thread::spawn(move || {
            let mut client = Client::connect(&database_url, NoTls).map_err(database_error)?;
            client
                .batch_execute(&format!(
                    "{IDENTITY_SCHEMA}\n{CHARACTER_SCHEMA}\n{USER_PRESENTATION_SCHEMA}\n{USER_OUTFIT_SCHEMA}\n{TABLECLOTH_SCHEMA}\n{MUSIC_SCHEMA}\n{RIICHI_MUSIC_SCHEMA}\n{MUSIC_SEED_SCHEMA}"
                ))
                .map_err(database_error)?;
            Ok(client)
        })
        .join()
        .map_err(|_| database_worker_failed())??;
        Ok(Self {
            client: Mutex::new(Some(client)),
        })
    }

    pub(crate) fn load_into(&self, store: &mut MemoryStore) -> Result<(), ApplicationError> {
        let (users, sessions, characters, tablecloths, music_tracks) =
            self.with_client(|client| {
                let users: Vec<StoredUserRow> = client
                    .query(
                        "SELECT u.id, u.version, u.login_name, u.password_hash, u.status, u.role,
                            u.nickname, u.selected_character_id, c.name AS selected_character_name,
                            u.selected_outfit_id, u.avatar_path, u.selected_tablecloth_id,
                            u.selected_lobby_music_id, u.selected_match_music_id,
                            u.selected_riichi_music_id
                     FROM mamahjong_users AS u
                     LEFT JOIN mamahjong_characters AS c ON c.id = u.selected_character_id
                     ORDER BY u.login_name",
                        &[],
                    )
                    .map_err(database_error)?
                    .into_iter()
                    .map(|row| {
                        (
                            row.get::<_, String>("id"),
                            row.get::<_, i64>("version"),
                            row.get::<_, String>("login_name"),
                            row.get::<_, String>("password_hash"),
                            row.get::<_, String>("status"),
                            row.get::<_, String>("role"),
                            row.get::<_, String>("nickname"),
                            row.get::<_, Option<String>>("selected_character_id"),
                            row.get::<_, Option<String>>("selected_character_name"),
                            row.get::<_, Option<String>>("selected_outfit_id"),
                            row.get::<_, Option<String>>("avatar_path"),
                            row.get::<_, Option<String>>("selected_tablecloth_id"),
                            row.get::<_, Option<String>>("selected_lobby_music_id"),
                            row.get::<_, Option<String>>("selected_match_music_id"),
                            row.get::<_, Option<String>>("selected_riichi_music_id"),
                        )
                    })
                    .collect();
                let sessions: Vec<StoredSessionRow> = client
                    .query("SELECT id, user_id, token FROM mamahjong_sessions", &[])
                    .map_err(database_error)?
                    .into_iter()
                    .map(|row| {
                        (
                            row.get::<_, String>("id"),
                            row.get::<_, String>("user_id"),
                            row.get::<_, String>("token"),
                        )
                    })
                    .collect();
                let characters: Vec<StoredCharacterRow> = client
                    .query(
                        "SELECT id, version, name, illustration_path, emotes_json, voices_json,
                            outfits_json, enabled, is_default
                     FROM mamahjong_characters
                     ORDER BY name, id",
                        &[],
                    )
                    .map_err(database_error)?
                    .into_iter()
                    .map(|row| {
                        (
                            row.get("id"),
                            row.get("version"),
                            row.get("name"),
                            row.get("illustration_path"),
                            row.get("emotes_json"),
                            row.get("voices_json"),
                            row.get("outfits_json"),
                            row.get("enabled"),
                            row.get("is_default"),
                        )
                    })
                    .collect();
                let tablecloths: Vec<StoredTableclothRow> = client
                    .query(
                        "SELECT id, version, name, texture_path, enabled, is_default
                     FROM mamahjong_tablecloths
                     ORDER BY name, id",
                        &[],
                    )
                    .map_err(database_error)?
                    .into_iter()
                    .map(|row| {
                        (
                            row.get("id"),
                            row.get("version"),
                            row.get("name"),
                            row.get("texture_path"),
                            row.get("enabled"),
                            row.get("is_default"),
                        )
                    })
                    .collect();
                let music_tracks: Vec<StoredMusicRow> = client
                .query(
                    "SELECT id, version, name, scene, audio_path, duration_ms, enabled, is_default
                     FROM mamahjong_music_tracks
                     ORDER BY name, id",
                    &[],
                )
                .map_err(database_error)?
                .into_iter()
                .map(|row| {
                    (
                        row.get("id"),
                        row.get("version"),
                        row.get("name"),
                        row.get("scene"),
                        row.get("audio_path"),
                        row.get("duration_ms"),
                        row.get("enabled"),
                        row.get("is_default"),
                    )
                })
                .collect();
                Ok((users, sessions, characters, tablecloths, music_tracks))
            })?;

        for (
            id,
            version,
            login_name,
            password_hash,
            status,
            role,
            nickname,
            selected_character_id,
            selected_character_name,
            selected_outfit_id,
            avatar_path,
            selected_tablecloth_id,
            selected_lobby_music_id,
            selected_match_music_id,
            selected_riichi_music_id,
        ) in users
        {
            let id = UserId::parse(id).map_err(corrupt_identity)?;
            let version = u64::try_from(version).map_err(corrupt_identity)?;
            let status = parse_status(&status)?;
            let role = parse_role(&role)?;
            let nickname = Nickname::parse(nickname).map_err(corrupt_identity)?;
            let selected_character = selected_character_id
                .zip(selected_character_name)
                .map(|(id, name)| CharacterSummary::new(id, name));
            let user = User::restore(
                id.clone(),
                version,
                login_name.clone(),
                status,
                role,
                nickname,
                selected_character,
                selected_outfit_id,
                avatar_path,
                selected_tablecloth_id,
                selected_lobby_music_id,
                selected_match_music_id,
                selected_riichi_music_id,
            );
            store.login_index.insert(login_name, id.clone());
            store.password_hashes.insert(id.clone(), password_hash);
            store.users.insert(id, user);
        }

        for (id, user_id, token) in sessions {
            let id = SessionId::parse(id).map_err(corrupt_identity)?;
            let user_id = UserId::parse(user_id).map_err(corrupt_identity)?;
            let session = Session::restore(id, user_id, token.clone());
            store.sessions.insert(token, session);
        }
        for (
            id,
            version,
            name,
            illustration_path,
            emotes_json,
            voices_json,
            outfits_json,
            enabled,
            is_default,
        ) in characters
        {
            let version = u64::try_from(version).map_err(corrupt_identity)?;
            let emotes: Vec<CharacterAsset> =
                serde_json::from_str(&emotes_json).map_err(corrupt_identity)?;
            let voices: Vec<CharacterVoice> =
                serde_json::from_str(&voices_json).map_err(corrupt_identity)?;
            let outfits: Vec<CharacterOutfit> =
                serde_json::from_str(&outfits_json).map_err(corrupt_identity)?;
            let character = Character::restore(
                SaveCharacter {
                    id: id.clone(),
                    name,
                    illustration_path,
                    emotes,
                    voices,
                    outfits,
                    enabled,
                    is_default,
                },
                version,
            )
            .map_err(corrupt_identity)?;
            store.characters.insert(id, character);
        }
        for (id, version, name, texture_path, enabled, is_default) in tablecloths {
            let version = u64::try_from(version).map_err(corrupt_identity)?;
            let tablecloth = Tablecloth::restore(
                SaveTablecloth {
                    id: id.clone(),
                    name,
                    texture_path,
                    enabled,
                    is_default,
                },
                version,
            )
            .map_err(corrupt_identity)?;
            store.tablecloths.insert(id, tablecloth);
        }
        for (id, version, name, scene, audio_path, duration_ms, enabled, is_default) in music_tracks
        {
            let version = u64::try_from(version).map_err(corrupt_identity)?;
            let track = MusicTrack::restore(
                SaveMusicTrack {
                    id: id.clone(),
                    name,
                    scene: MusicScene::parse(&scene).map_err(corrupt_identity)?,
                    audio_path,
                    duration_ms: u64::try_from(duration_ms).map_err(corrupt_identity)?,
                    enabled,
                    is_default,
                },
                version,
            )
            .map_err(corrupt_identity)?;
            store.music_tracks.insert(id, track);
        }
        Ok(())
    }

    pub(crate) fn upsert_character(&self, character: &Character) -> Result<(), ApplicationError> {
        let emotes = serde_json::to_string(character.emotes()).map_err(database_json_error)?;
        let voices = serde_json::to_string(character.voices()).map_err(database_json_error)?;
        let outfits = serde_json::to_string(character.outfits()).map_err(database_json_error)?;
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            if character.is_default() {
                transaction
                    .execute(
                        "UPDATE mamahjong_characters SET is_default = FALSE
                         WHERE is_default = TRUE AND id <> $1",
                        &[&character.id()],
                    )
                    .map_err(database_error)?;
            }
            transaction
                .execute(
                    "INSERT INTO mamahjong_characters
                        (id, version, name, illustration_path, emotes_json, voices_json,
                         outfits_json, enabled, is_default)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     ON CONFLICT (id) DO UPDATE SET
                         version = EXCLUDED.version,
                         name = EXCLUDED.name,
                         illustration_path = EXCLUDED.illustration_path,
                         emotes_json = EXCLUDED.emotes_json,
                         voices_json = EXCLUDED.voices_json,
                         outfits_json = EXCLUDED.outfits_json,
                         enabled = EXCLUDED.enabled,
                         is_default = EXCLUDED.is_default",
                    &[
                        &character.id(),
                        &version_as_i64(character.version())?,
                        &character.name(),
                        &character.illustration_path(),
                        &emotes,
                        &voices,
                        &outfits,
                        &character.enabled(),
                        &character.is_default(),
                    ],
                )
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)
        })
    }

    pub(crate) fn delete_character(&self, character_id: &str) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM mamahjong_characters WHERE id = $1",
                    &[&character_id],
                )
                .map(|_| ())
                .map_err(database_error)
        })
    }

    pub(crate) fn upsert_tablecloth(
        &self,
        tablecloth: &Tablecloth,
    ) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            if tablecloth.is_default() {
                transaction
                    .execute(
                        "UPDATE mamahjong_tablecloths SET is_default = FALSE
                         WHERE is_default = TRUE AND id <> $1",
                        &[&tablecloth.id()],
                    )
                    .map_err(database_error)?;
            }
            transaction
                .execute(
                    "INSERT INTO mamahjong_tablecloths
                        (id, version, name, texture_path, enabled, is_default)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (id) DO UPDATE SET
                         version = EXCLUDED.version,
                         name = EXCLUDED.name,
                         texture_path = EXCLUDED.texture_path,
                         enabled = EXCLUDED.enabled,
                         is_default = EXCLUDED.is_default",
                    &[
                        &tablecloth.id(),
                        &version_as_i64(tablecloth.version())?,
                        &tablecloth.name(),
                        &tablecloth.texture_path(),
                        &tablecloth.enabled(),
                        &tablecloth.is_default(),
                    ],
                )
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)
        })
    }

    pub(crate) fn delete_tablecloth(&self, tablecloth_id: &str) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM mamahjong_tablecloths WHERE id = $1",
                    &[&tablecloth_id],
                )
                .map(|_| ())
                .map_err(database_error)
        })
    }

    pub(crate) fn upsert_music_track(&self, track: &MusicTrack) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            if track.is_default() {
                // 默认只在自己那个场景里唯一：换了大厅默认曲不该动对局的。
                transaction
                    .execute(
                        "UPDATE mamahjong_music_tracks SET is_default = FALSE
                         WHERE is_default = TRUE AND scene = $1 AND id <> $2",
                        &[&track.scene().as_str(), &track.id()],
                    )
                    .map_err(database_error)?;
            }
            transaction
                .execute(
                    "INSERT INTO mamahjong_music_tracks
                        (id, version, name, scene, audio_path, duration_ms, enabled, is_default)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     ON CONFLICT (id) DO UPDATE SET
                         version = EXCLUDED.version,
                         name = EXCLUDED.name,
                         scene = EXCLUDED.scene,
                         audio_path = EXCLUDED.audio_path,
                         duration_ms = EXCLUDED.duration_ms,
                         enabled = EXCLUDED.enabled,
                         is_default = EXCLUDED.is_default",
                    &[
                        &track.id(),
                        &version_as_i64(track.version())?,
                        &track.name(),
                        &track.scene().as_str(),
                        &track.audio_path(),
                        &version_as_i64(track.duration_ms())?,
                        &track.enabled(),
                        &track.is_default(),
                    ],
                )
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)
        })
    }

    pub(crate) fn delete_music_track(&self, track_id: &str) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM mamahjong_music_tracks WHERE id = $1",
                    &[&track_id],
                )
                .map(|_| ())
                .map_err(database_error)
        })
    }

    pub(crate) fn insert_user(
        &self,
        user: &User,
        password_hash: &str,
        session: Option<&Session>,
    ) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            transaction
                .execute(
                    "INSERT INTO mamahjong_users
                        (id, version, login_name, password_hash, status, role, nickname,
                         selected_character_id, selected_outfit_id, avatar_path,
                         selected_tablecloth_id, selected_lobby_music_id,
                         selected_match_music_id, selected_riichi_music_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
                    &[
                        &user.id().as_str(),
                        &version_as_i64(user.version())?,
                        &user.login_name(),
                        &password_hash,
                        &status_text(user.status()),
                        &role_text(user.role()),
                        &user.profile().nickname().as_str(),
                        &user
                            .profile()
                            .selected_character()
                            .map(CharacterSummary::id),
                        &user.profile().selected_outfit_id(),
                        &user.profile().avatar_path(),
                        &user.profile().selected_tablecloth_id(),
                        &user.profile().selected_lobby_music_id(),
                        &user.profile().selected_match_music_id(),
                        &user.profile().selected_riichi_music_id(),
                    ],
                )
                .map_err(database_error)?;
            if let Some(session) = session {
                insert_session(&mut transaction, session)?;
            }
            transaction.commit().map_err(database_error)
        })
    }

    /// Inserts a session without removing any existing ones.
    pub(crate) fn insert_session_only(&self, session: &Session) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            insert_session(&mut transaction, session)?;
            transaction.commit().map_err(database_error)
        })
    }

    /// Deletes every session for `user_id` except the one matching `keep_token`.
    pub(crate) fn revoke_other_sessions(
        &self,
        user_id: &UserId,
        keep_token: &str,
    ) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM mamahjong_sessions WHERE user_id = $1 AND token <> $2",
                    &[&user_id.as_str(), &keep_token],
                )
                .map(|_| ())
                .map_err(database_error)
        })
    }

    pub(crate) fn update_user(
        &self,
        user: &User,
        revoke_sessions: bool,
    ) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            let updated = transaction
                .execute(
                    "UPDATE mamahjong_users
                     SET version = $2, status = $3, role = $4, nickname = $5,
                         selected_character_id = $6, selected_outfit_id = $7,
                         avatar_path = $8, selected_tablecloth_id = $9,
                         selected_lobby_music_id = $10, selected_match_music_id = $11,
                         selected_riichi_music_id = $12
                     WHERE id = $1",
                    &[
                        &user.id().as_str(),
                        &version_as_i64(user.version())?,
                        &status_text(user.status()),
                        &role_text(user.role()),
                        &user.profile().nickname().as_str(),
                        &user
                            .profile()
                            .selected_character()
                            .map(CharacterSummary::id),
                        &user.profile().selected_outfit_id(),
                        &user.profile().avatar_path(),
                        &user.profile().selected_tablecloth_id(),
                        &user.profile().selected_lobby_music_id(),
                        &user.profile().selected_match_music_id(),
                        &user.profile().selected_riichi_music_id(),
                    ],
                )
                .map_err(database_error)?;
            if updated != 1 {
                return Err(database_inconsistent());
            }
            if revoke_sessions {
                transaction
                    .execute(
                        "DELETE FROM mamahjong_sessions WHERE user_id = $1",
                        &[&user.id().as_str()],
                    )
                    .map_err(database_error)?;
            }
            transaction.commit().map_err(database_error)
        })
    }

    pub(crate) fn update_password(
        &self,
        user_id: &UserId,
        password_hash: &str,
    ) -> Result<(), ApplicationError> {
        self.with_client(|client| {
            let mut transaction = client.transaction().map_err(database_error)?;
            let updated = transaction
                .execute(
                    "UPDATE mamahjong_users SET password_hash = $2 WHERE id = $1",
                    &[&user_id.as_str(), &password_hash],
                )
                .map_err(database_error)?;
            if updated != 1 {
                return Err(database_inconsistent());
            }
            transaction
                .execute(
                    "DELETE FROM mamahjong_sessions WHERE user_id = $1",
                    &[&user_id.as_str()],
                )
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)
        })
    }

    fn with_client<T>(
        &self,
        operation: impl FnOnce(&mut Client) -> Result<T, ApplicationError> + Send,
    ) -> Result<T, ApplicationError>
    where
        T: Send,
    {
        thread::scope(|scope| {
            scope
                .spawn(move || {
                    let mut client = self.client.lock().map_err(|_| {
                        ApplicationError::new(
                            ErrorCode::Internal,
                            "identity database client lock is poisoned",
                        )
                    })?;
                    let client = client.as_mut().ok_or_else(database_worker_failed)?;
                    operation(client)
                })
                .join()
                .map_err(|_| database_worker_failed())?
        })
    }
}

impl Drop for PostgresIdentityStore {
    fn drop(&mut self) {
        let Ok(client) = self.client.get_mut() else {
            return;
        };
        let Some(client) = client.take() else {
            return;
        };
        let _ = thread::spawn(move || drop(client)).join();
    }
}

fn insert_session(
    transaction: &mut postgres::Transaction<'_>,
    session: &Session,
) -> Result<(), ApplicationError> {
    transaction
        .execute(
            "INSERT INTO mamahjong_sessions (token, id, user_id) VALUES ($1, $2, $3)",
            &[
                &session.token(),
                &session.id().as_str(),
                &session.user_id().as_str(),
            ],
        )
        .map(|_| ())
        .map_err(database_error)
}

const fn status_text(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Suspended => "suspended",
    }
}

fn parse_status(value: &str) -> Result<AccountStatus, ApplicationError> {
    match value {
        "active" => Ok(AccountStatus::Active),
        "suspended" => Ok(AccountStatus::Suspended),
        _ => Err(database_inconsistent()),
    }
}

const fn role_text(role: AccountRole) -> &'static str {
    match role {
        AccountRole::Player => "player",
        AccountRole::Administrator => "administrator",
    }
}

fn parse_role(value: &str) -> Result<AccountRole, ApplicationError> {
    match value {
        "player" => Ok(AccountRole::Player),
        "administrator" => Ok(AccountRole::Administrator),
        _ => Err(database_inconsistent()),
    }
}

fn version_as_i64(version: u64) -> Result<i64, ApplicationError> {
    i64::try_from(version).map_err(|_| database_inconsistent())
}

fn database_error(error: postgres::Error) -> ApplicationError {
    ApplicationError::new(
        ErrorCode::Internal,
        format!("identity database operation failed: {error}"),
    )
}

fn database_json_error(error: serde_json::Error) -> ApplicationError {
    ApplicationError::new(
        ErrorCode::Internal,
        format!("character metadata serialization failed: {error}"),
    )
}

fn corrupt_identity(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::new(
        ErrorCode::Internal,
        format!("identity database contains invalid data: {error}"),
    )
}

fn database_inconsistent() -> ApplicationError {
    ApplicationError::new(
        ErrorCode::Internal,
        "identity database contains inconsistent data",
    )
}

fn database_worker_failed() -> ApplicationError {
    ApplicationError::new(ErrorCode::Internal, "identity database worker failed")
}
