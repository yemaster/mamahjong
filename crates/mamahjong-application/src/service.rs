use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use mahjong_core::{MatchId, RoomId, UserId};
use mahjong_riichi::{RiichiVariant, RoomRuleRequest};

use crate::game::GameRuntime;
use crate::store::MemoryStore;
use crate::{
    AccountStatus, ApplicationError, ErrorCode, GameRuleSnapshot, Nickname, ObserverMatch, Room,
    RoomVisibility, Session, SubmitGameCommand, User,
};

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
pub enum RoomRuleSelection {
    Riichi {
        variant: RiichiVariant,
        request: RoomRuleRequest,
    },
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

#[derive(Clone, Default)]
pub struct Application {
    store: Arc<RwLock<MemoryStore>>,
}

impl Application {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, command: RegisterUser) -> Result<(User, Session), ApplicationError> {
        let login_name = canonical_login_name(&command.login_name)?;
        validate_password(&command.password)?;
        let nickname = Nickname::parse(command.nickname)?;
        let password_hash = hash_password(&command.password)?;
        let mut store = self.write_store()?;
        if store.login_index.contains_key(&login_name) {
            return Err(ApplicationError::new(
                ErrorCode::LoginNameTaken,
                "login name is already registered",
            ));
        }
        let user = User::new(login_name.clone(), nickname);
        let session = new_session(user.id().clone())?;
        store.login_index.insert(login_name, user.id().clone());
        store
            .password_hashes
            .insert(user.id().clone(), password_hash);
        store
            .sessions
            .insert(session.token().to_owned(), session.clone());
        store.users.insert(user.id().clone(), user.clone());
        Ok((user, session))
    }

    pub fn login(
        &self,
        login_name: &str,
        password: &str,
    ) -> Result<(User, Session), ApplicationError> {
        let login_name = canonical_login_name(login_name)?;
        let (user_id, password_hash, user) = {
            let store = self.read_store()?;
            let user_id = store
                .login_index
                .get(&login_name)
                .ok_or_else(invalid_credentials)?
                .clone();
            let password_hash = store
                .password_hashes
                .get(&user_id)
                .ok_or_else(internal_error)?
                .clone();
            let user = store
                .users
                .get(&user_id)
                .ok_or_else(internal_error)?
                .clone();
            (user_id, password_hash, user)
        };
        verify_password(password, &password_hash)?;
        if user.status() != AccountStatus::Active {
            return Err(ApplicationError::new(
                ErrorCode::UserUnavailable,
                "user account is unavailable",
            ));
        }
        let session = new_session(user_id)?;
        self.write_store()?
            .sessions
            .insert(session.token().to_owned(), session.clone());
        Ok((user, session))
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
        let user = store.users.get_mut(actor).ok_or_else(internal_error)?;
        user.rename(nickname);
        Ok(user.clone())
    }

    pub fn create_room(
        &self,
        actor: &UserId,
        command: CreateRoom,
    ) -> Result<Room, ApplicationError> {
        let name = validate_room_name(command.name)?;
        let snapshot = resolve_rules(command.rules)?;
        let mut store = self.write_store()?;
        let owner = store.users.get(actor).ok_or_else(internal_error)?;
        let room = Room::new(
            actor.clone(),
            owner.profile().nickname().as_str().to_owned(),
            name,
            command.visibility,
            snapshot,
        );
        store.rooms.insert(room.id().clone(), room.clone());
        Ok(room)
    }

    pub fn list_rooms(&self) -> Result<Vec<Room>, ApplicationError> {
        let store = self.read_store()?;
        let mut rooms: Vec<_> = store
            .rooms
            .values()
            .filter(|room| room.visibility() == RoomVisibility::Public)
            .cloned()
            .collect();
        rooms.sort_unstable_by(|left, right| left.id().cmp(right.id()));
        Ok(rooms)
    }

    pub fn room(&self, room_id: &RoomId) -> Result<Room, ApplicationError> {
        self.read_store()?
            .rooms
            .get(room_id)
            .cloned()
            .ok_or_else(room_not_found)
    }

    pub fn join_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
    ) -> Result<Room, ApplicationError> {
        let mut store = self.write_store()?;
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
        let room = store.rooms.get_mut(room_id).ok_or_else(room_not_found)?;
        ensure_version(room, expected_version)?;
        room.leave(actor)?;
        Ok(room.clone())
    }

    pub fn start_room(
        &self,
        actor: &UserId,
        room_id: &RoomId,
        expected_version: u64,
    ) -> Result<(Room, MatchId), ApplicationError> {
        let mut store = self.write_store()?;
        let mut room = store.rooms.get(room_id).ok_or_else(room_not_found)?.clone();
        ensure_version(&room, expected_version)?;
        let match_id = room.start(actor)?;
        let game = GameRuntime::start(&room, match_id.clone())?;
        store.rooms.insert(room_id.clone(), room.clone());
        store.matches.insert(match_id.clone(), game);
        Ok((room, match_id))
    }

    pub fn match_view(
        &self,
        actor: &UserId,
        match_id: &MatchId,
    ) -> Result<ObserverMatch, ApplicationError> {
        self.read_store()?
            .matches
            .get(match_id)
            .ok_or_else(match_not_found)?
            .view(actor)
    }

    pub fn submit_game_command(
        &self,
        actor: &UserId,
        match_id: &MatchId,
        command: SubmitGameCommand,
    ) -> Result<ObserverMatch, ApplicationError> {
        let mut store = self.write_store()?;
        let game = store
            .matches
            .get_mut(match_id)
            .ok_or_else(match_not_found)?;
        game.execute(actor, command)?;
        game.view(actor)
    }

    fn read_store(&self) -> Result<RwLockReadGuard<'_, MemoryStore>, ApplicationError> {
        self.store.read().map_err(|_| internal_error())
    }

    fn write_store(&self) -> Result<RwLockWriteGuard<'_, MemoryStore>, ApplicationError> {
        self.store.write().map_err(|_| internal_error())
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

fn validate_password(password: &str) -> Result<(), ApplicationError> {
    if !(10..=128).contains(&password.len()) {
        return Err(ApplicationError::new(
            ErrorCode::InvalidPassword,
            "password must contain 10 to 128 bytes",
        ));
    }
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
        Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdateProfile, UpdateRoom,
    };
    use crate::{ErrorCode, GameCommand, RoomVisibility, SubmitGameCommand};

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
    fn registration_hashes_password_and_returns_redacted_session() {
        let application = Application::new();
        let (user, session) = register(&application, "one");

        assert_eq!(user.profile().nickname().as_str(), "雀士one");
        assert!(!format!("{session:?}").contains(session.token()));
        assert_eq!(
            application
                .login("PLAYER_ONE", "correct horse battery staple")
                .expect("login")
                .0
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
                .start_room(owner.id(), room.id(), room.version())
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
            .start_room(owner.id(), room.id(), room.version())
            .expect("start");

        assert_eq!(started.active_match_id(), Some(&match_id));
        assert_eq!(started.lifecycle(), crate::RoomLifecycle::Playing);
        assert_eq!(
            application
                .start_room(owner.id(), room.id(), started.version())
                .expect_err("already playing")
                .code(),
            ErrorCode::RoomPlaying
        );

        let players = [&owner, &guest_one, &guest_two];
        let mut view = application
            .match_view(owner.id(), &match_id)
            .expect("initial match view");
        for _ in 0..500 {
            if view.hand_index() > 0 {
                break;
            }
            match view.phase() {
                mahjong_riichi::HandPhase::AwaitingTurnAction { seat }
                | mahjong_riichi::HandPhase::AwaitingDiscard { seat } => {
                    let actor = players[usize::from(seat.index())];
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
                        )
                        .expect("discard");
                }
                mahjong_riichi::HandPhase::AwaitingResponses { trigger_seat } => {
                    for (seat_index, actor) in players.iter().enumerate() {
                        if seat_index == usize::from(trigger_seat.index()) {
                            continue;
                        }
                        view = application
                            .submit_game_command(
                                actor.id(),
                                &match_id,
                                SubmitGameCommand {
                                    expected_version: view.version(),
                                    command: GameCommand::Pass,
                                },
                            )
                            .expect("pass");
                    }
                }
                mahjong_riichi::HandPhase::Ended { .. } => {
                    panic!("a completed non-terminal hand must start the next hand")
                }
            }
        }

        assert_eq!(view.hand_index(), 1);
        assert_eq!(view.progress().round_number().value(), 2);
        assert!(view.event_sequence() > 200);
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
            .start_room(players[0].id(), room.id(), room.version())
            .expect("start");
        let mut view = application
            .match_view(players[0].id(), &match_id)
            .expect("initial view");

        for _ in 0..5_000 {
            if view.result().is_some() {
                break;
            }
            match view.phase() {
                mahjong_riichi::HandPhase::AwaitingTurnAction { seat }
                | mahjong_riichi::HandPhase::AwaitingDiscard { seat } => {
                    let actor = &players[usize::from(seat.index())];
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
                        )
                        .expect("discard");
                }
                mahjong_riichi::HandPhase::AwaitingResponses { trigger_seat } => {
                    for (index, actor) in players.iter().enumerate() {
                        if index == usize::from(trigger_seat.index()) {
                            continue;
                        }
                        view = application
                            .submit_game_command(
                                actor.id(),
                                &match_id,
                                SubmitGameCommand {
                                    expected_version: view.version(),
                                    command: GameCommand::Pass,
                                },
                            )
                            .expect("pass");
                    }
                }
                mahjong_riichi::HandPhase::Ended { .. } => {
                    assert!(
                        view.result().is_some(),
                        "only the terminal hand stays ended"
                    );
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
    }
}
