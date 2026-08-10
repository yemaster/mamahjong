use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};

use crate::api::ApiClient;
use crate::model::{
    MatchPhase, MatchView, MatchmakingTicketView, ReactionOptionView, RoomView, UserView,
};
use crate::rules::{CreateRoomForm, RuleSetCatalog};
use crate::stream::{MatchStream, SeatCountdown, StreamEvent};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    Login,
    Register,
}

#[derive(Debug)]
pub struct AuthForm {
    pub mode: AuthMode,
    pub active_field: usize,
    pub login_name: String,
    pub password: String,
    pub nickname: String,
}

#[derive(Debug, Default)]
pub struct RoomBrowser {
    pub rooms: Vec<RoomView>,
    pub selected: usize,
}

#[derive(Debug)]
pub struct RoomScreen {
    pub room: RoomView,
    pub rules_scroll: u16,
}

impl RoomScreen {
    pub fn new(room: RoomView) -> Self {
        Self {
            room,
            rules_scroll: 0,
        }
    }
}

#[derive(Debug)]
pub struct GameScreen {
    pub view: MatchView,
    pub selected_tile: usize,
    pub marked_tile_ids: Vec<u16>,
    pub countdowns: Vec<SeatCountdown>,
    pub online: Vec<bool>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Auth(AuthForm),
    Rooms(RoomBrowser),
    CreateRoom(Box<CreateRoomForm>),
    Matchmaking(MatchmakingTicketView),
    Room(RoomScreen),
    Game(GameScreen),
}

pub struct App {
    pub screen: Screen,
    pub user: Option<UserView>,
    pub status: String,
    pub quit: bool,
    api: ApiClient,
    rule_catalog: Option<RuleSetCatalog>,
    last_poll: Instant,
    stream: Option<MatchStream>,
}

enum Action {
    ShowStatus(String),
    Authenticate {
        mode: AuthMode,
        login_name: String,
        password: String,
        nickname: String,
    },
    RefreshRooms,
    OpenCreateRoom,
    EnterMatchmaking {
        variant: &'static str,
    },
    RefreshMatchmaking,
    CancelMatchmaking,
    BackToRooms,
    CreateRoom {
        payload: Value,
    },
    JoinRoom {
        id: String,
        version: u64,
    },
    RefreshRoom,
    Ready,
    StartRoom,
    LeaveRoom,
    ReturnToRoom,
    RefreshGame,
    GameCommand {
        name: &'static str,
        payload: Option<Value>,
    },
}

impl App {
    pub fn new(base_url: String) -> Result<Self, crate::model::ApiFailure> {
        Ok(Self {
            screen: Screen::Auth(AuthForm {
                mode: AuthMode::Login,
                active_field: 0,
                login_name: String::new(),
                password: String::new(),
                nickname: String::new(),
            }),
            user: None,
            status: "未登录".to_owned(),
            quit: false,
            api: ApiClient::new(base_url)?,
            rule_catalog: None,
            last_poll: Instant::now(),
            stream: None,
        })
    }

    pub async fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let action = self.reduce_key(key);
        if let Some(action) = action {
            self.perform(action).await;
        }
    }

    pub async fn poll_if_due(&mut self) -> bool {
        // Drain stream events without waiting — they arrive between frames.
        if let Screen::Game(_) = &self.screen {
            let mut dirty = false;
            if let Some(stream) = &mut self.stream {
                for event in stream.drain() {
                    dirty = true;
                    match event {
                        StreamEvent::EventsArrived => {
                            self.last_poll = Instant::now();
                            self.perform(Action::RefreshGame).await;
                        }
                        StreamEvent::Clock { seats } => {
                            if let Screen::Game(ref mut game) = self.screen {
                                game.countdowns = seats;
                            }
                        }
                        StreamEvent::Presence { seats } => {
                            if let Screen::Game(ref mut game) = self.screen {
                                game.online = seats.iter().map(|s| s.online).collect();
                            }
                        }
                        StreamEvent::Disconnected => {
                            self.status = "WebSocket 已断开，退回轮询".to_owned();
                        }
                        StreamEvent::Reconnected { .. } => {
                            self.status = "WebSocket 已重连".to_owned();
                            // Refresh immediately to catch up.
                            self.last_poll = Instant::now();
                            self.perform(Action::RefreshGame).await;
                        }
                    }
                }
            }
            // Fall back to HTTP polling when the stream is disconnected or
            // no events have arrived recently.
            let connected = self
                .stream
                .as_ref()
                .is_some_and(|stream| stream.is_connected());
            if !connected && self.last_poll.elapsed() >= POLL_INTERVAL {
                self.last_poll = Instant::now();
                self.perform(Action::RefreshGame).await;
                return true;
            }
            return dirty;
        }

        if self.last_poll.elapsed() < POLL_INTERVAL {
            return false;
        }
        self.last_poll = Instant::now();
        let action = match self.screen {
            Screen::Room(_) => Some(Action::RefreshRoom),
            Screen::Matchmaking(_) => Some(Action::RefreshMatchmaking),
            _ => None,
        };
        if let Some(action) = action {
            self.perform(action).await;
            true
        } else {
            false
        }
    }

    fn reduce_key(&mut self, key: KeyEvent) -> Option<Action> {
        match &mut self.screen {
            Screen::Auth(form) => auth_key(form, key, &mut self.quit),
            Screen::Rooms(browser) => rooms_key(browser, key, &mut self.quit),
            Screen::CreateRoom(form) => create_room_key(form, key),
            Screen::Matchmaking(ticket) => matchmaking_key(ticket, key),
            Screen::Room(screen) => room_key(screen, key),
            Screen::Game(game) => game_key(game, key, &mut self.quit),
        }
    }

    async fn perform(&mut self, action: Action) {
        let result = match action {
            Action::ShowStatus(message) => {
                self.status = message;
                Ok(())
            }
            Action::Authenticate {
                mode,
                login_name,
                password,
                nickname,
            } => {
                let response = match mode {
                    AuthMode::Login => self.api.login(&login_name, &password).await,
                    AuthMode::Register => {
                        self.api.register(&login_name, &password, &nickname).await
                    }
                };
                match response {
                    Ok(response) => {
                        self.api.set_token(response.session.token);
                        self.status = format!("欢迎，{}", response.user.profile.nickname);
                        self.user = Some(response.user);
                        match self.api.rule_sets().await {
                            Ok(catalog) => {
                                self.rule_catalog = Some(catalog);
                                self.load_rooms().await
                            }
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            }
            Action::RefreshRooms => self.load_rooms().await,
            Action::OpenCreateRoom => match self.rule_catalog.clone() {
                Some(catalog) => match CreateRoomForm::new(catalog) {
                    Ok(form) => {
                        self.screen = Screen::CreateRoom(Box::new(form));
                        Ok(())
                    }
                    Err(error) => Err(error),
                },
                None => Err(crate::model::ApiFailure {
                    code: "client.invalid_response".to_owned(),
                    message: "规则目录尚未载入".to_owned(),
                }),
            },
            Action::EnterMatchmaking { variant } => match self.api.enter_matchmaking(variant).await
            {
                Ok(ticket) => self.open_matchmaking(ticket).await,
                Err(error) => Err(error),
            },
            Action::RefreshMatchmaking => self.refresh_matchmaking().await,
            Action::CancelMatchmaking => {
                let Screen::Matchmaking(ticket) = &self.screen else {
                    return;
                };
                match self.api.cancel_matchmaking(&ticket.id).await {
                    Ok(_) => {
                        self.status = "已取消匹配".to_owned();
                        self.load_rooms().await
                    }
                    Err(error) => Err(error),
                }
            }
            Action::BackToRooms => self.load_rooms().await,
            Action::CreateRoom { payload } => self.api.create_room(payload).await.map(|room| {
                self.status = "房间已创建".to_owned();
                self.screen = Screen::Room(RoomScreen::new(room));
            }),
            Action::JoinRoom { id, version } => {
                self.api.join_room(&id, version).await.map(|room| {
                    self.status = "已加入房间".to_owned();
                    self.screen = Screen::Room(RoomScreen::new(room));
                })
            }
            Action::RefreshRoom => self.refresh_room().await,
            Action::Ready => self.toggle_ready().await,
            Action::StartRoom => self.start_room().await,
            Action::LeaveRoom => self.leave_room().await,
            Action::ReturnToRoom => {
                let Screen::Game(game) = &self.screen else {
                    return;
                };
                match self.api.room(&game.view.room_id).await {
                    Ok(room) => {
                        self.status = "已返回房间".to_owned();
                        self.screen = Screen::Room(RoomScreen::new(room));
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            Action::RefreshGame => self.refresh_game().await,
            Action::GameCommand { name, payload } => self.submit_game_command(name, payload).await,
        };
        if let Err(error) = result {
            self.status = error.to_string();
        }
    }

    fn connect_stream(&mut self, match_id: &str) {
        let Some(token) = self.api.token() else {
            return;
        };
        let after_seq = self.stream.as_ref().map_or(0, |stream| stream.last_seq());
        self.stream = Some(MatchStream::connect(
            self.api.base_url().to_owned(),
            token.to_string(),
            match_id.to_owned(),
            after_seq,
        ));
    }

    async fn entering_game(
        &mut self,
        match_id: &str,
        view: MatchView,
    ) -> Result<GameScreen, crate::model::ApiFailure> {
        // Preserve countdowns and online if already in a game for the same match.
        let (countdowns, online) = match &self.screen {
            Screen::Game(game) if game.view.id == match_id => {
                (game.countdowns.clone(), game.online.clone())
            }
            _ => (Vec::new(), vec![true; view.players.len()]),
        };
        self.connect_stream(match_id);
        // 终端客户端没有素材要load，但这一步必须报到：全场报到之前服务端一条
        // 命令都不收，一直不报到整局会被判超时作废。
        let view = if view.needs_assets_ready() {
            self.api
                .game_command(match_id, view.version, "game.assets_ready", None)
                .await?
        } else {
            view
        };
        Ok(GameScreen {
            view,
            selected_tile: 0,
            marked_tile_ids: Vec::new(),
            countdowns,
            online,
        })
    }

    async fn load_rooms(&mut self) -> Result<(), crate::model::ApiFailure> {
        self.stream = None;
        let response = self.api.rooms().await?;
        self.screen = Screen::Rooms(RoomBrowser {
            rooms: response.rooms,
            selected: 0,
        });
        Ok(())
    }

    async fn refresh_room(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(current) = &self.screen else {
            return Ok(());
        };
        let rules_scroll = current.rules_scroll;
        let room = self.api.room(&current.room.id).await?;
        if let Some(match_id) = &room.active_match_id.clone() {
            let view = self.api.match_view(match_id).await?;
            let screen = self.entering_game(match_id, view).await?;
            self.screen = Screen::Game(screen);
        } else {
            self.screen = Screen::Room(RoomScreen { room, rules_scroll });
        }
        Ok(())
    }

    async fn refresh_matchmaking(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Matchmaking(current) = &self.screen else {
            return Ok(());
        };
        let ticket = self.api.matchmaking_ticket(&current.id).await?;
        self.open_matchmaking(ticket).await
    }

    async fn open_matchmaking(
        &mut self,
        ticket: MatchmakingTicketView,
    ) -> Result<(), crate::model::ApiFailure> {
        if let Some(match_id) = &ticket.match_id.clone() {
            let view = self.api.match_view(match_id).await?;
            self.status = "匹配成功，对局开始".to_owned();
            let screen = self.entering_game(match_id, view).await?;
            self.screen = Screen::Game(screen);
        } else {
            self.status = "正在匹配".to_owned();
            self.screen = Screen::Matchmaking(ticket);
        }
        Ok(())
    }

    async fn toggle_ready(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(screen) = &self.screen else {
            return Ok(());
        };
        let Some(user) = &self.user else {
            return Ok(());
        };
        let rules_scroll = screen.rules_scroll;
        let room = &screen.room;
        let ready = room
            .members
            .iter()
            .find(|member| member.user_id == user.id)
            .is_some_and(|member| member.ready);
        let room = self.api.set_ready(&room.id, room.version, !ready).await?;
        self.status = if ready {
            "已取消准备".to_owned()
        } else {
            "已准备".to_owned()
        };
        self.screen = Screen::Room(RoomScreen { room, rules_scroll });
        Ok(())
    }

    async fn start_room(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(screen) = &self.screen else {
            return Ok(());
        };
        let room = &screen.room;
        let started = self.api.start_room(&room.id, room.version).await?;
        let view = self.api.match_view(&started.match_id).await?;
        self.status = "对局开始".to_owned();
        let screen = self.entering_game(&started.match_id, view).await?;
        self.screen = Screen::Game(screen);
        Ok(())
    }

    async fn leave_room(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(screen) = &self.screen else {
            return Ok(());
        };
        let room = &screen.room;
        self.api.leave_room(&room.id, room.version).await?;
        self.load_rooms().await
    }

    async fn refresh_game(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Game(game) = &self.screen else {
            return Ok(());
        };
        let old_hand = game.view.hand_index;
        let countdowns = game.countdowns.clone();
        let online = game.online.clone();
        let view = self.api.match_view(&game.view.id).await?;
        if view.terminated_by_asset_timeout {
            self.stream = None;
            self.status = "有玩家出现网络问题，对局已终止".to_owned();
            return self.load_rooms().await;
        }
        let selected = clamp_selection(game.selected_tile, &view);
        let marked_tile_ids = retained_marks(&game.marked_tile_ids, &view, old_hand);
        if let Some(stream) = &mut self.stream {
            stream.set_last_seq(view.event_sequence);
        }
        self.screen = Screen::Game(GameScreen {
            view,
            selected_tile: selected,
            marked_tile_ids,
            countdowns,
            online,
        });
        Ok(())
    }

    async fn submit_game_command(
        &mut self,
        name: &'static str,
        payload: Option<Value>,
    ) -> Result<(), crate::model::ApiFailure> {
        let Screen::Game(game) = &self.screen else {
            return Ok(());
        };
        let (match_id, version) = (game.view.id.clone(), game.view.version);

        // Send via WebSocket when connected, then refresh via HTTP for the
        // authoritative state; fall back to the HTTP command endpoint otherwise.
        if let Some(stream) = &self.stream {
            if stream.is_connected() {
                let ws_frame = serde_json::to_string(&json!({
                    "kind": "command",
                    "command_id": command_id(),
                    "stream": format!("match_{match_id}"),
                    "expected_version": version,
                    "name": name,
                    "payload": payload
                }))
                .expect("command json");
                stream.send_command(ws_frame);
            }
        }
        // Always refresh via HTTP so the display updates before the next
        // event-driven refresh arrives.
        let view = if self.stream.as_ref().is_some_and(|s| s.is_connected()) {
            self.api.match_view(&match_id).await?
        } else {
            self.api
                .game_command(&match_id, version, name, payload)
                .await?
        };

        let selected = clamp_selection(game.selected_tile, &view);
        let countdowns = game.countdowns.clone();
        let online = game.online.clone();
        if view.result.is_some() {
            self.status = "整场结束".to_owned();
        } else {
            self.status = command_status(name).to_owned();
        }
        if let Some(stream) = &mut self.stream {
            stream.set_last_seq(view.event_sequence);
        }
        self.screen = Screen::Game(GameScreen {
            view,
            selected_tile: selected,
            marked_tile_ids: Vec::new(),
            countdowns,
            online,
        });
        Ok(())
    }
}

fn auth_key(form: &mut AuthForm, key: KeyEvent, quit: &mut bool) -> Option<Action> {
    match key.code {
        KeyCode::Esc => *quit = true,
        KeyCode::F(2) => {
            form.mode = match form.mode {
                AuthMode::Login => AuthMode::Register,
                AuthMode::Register => AuthMode::Login,
            };
            form.active_field = 0;
        }
        KeyCode::Tab | KeyCode::Down => {
            let fields = if form.mode == AuthMode::Register {
                3
            } else {
                2
            };
            form.active_field = (form.active_field + 1) % fields;
        }
        KeyCode::BackTab | KeyCode::Up => {
            let fields = if form.mode == AuthMode::Register {
                3
            } else {
                2
            };
            form.active_field = (form.active_field + fields - 1) % fields;
        }
        KeyCode::Backspace => active_auth_text(form).pop().map(|_| ())?,
        KeyCode::Char(character) => active_auth_text(form).push(character),
        KeyCode::Enter => {
            return Some(Action::Authenticate {
                mode: form.mode,
                login_name: form.login_name.clone(),
                password: form.password.clone(),
                nickname: form.nickname.clone(),
            });
        }
        _ => {}
    }
    None
}

fn active_auth_text(form: &mut AuthForm) -> &mut String {
    match form.active_field {
        0 => &mut form.login_name,
        1 => &mut form.password,
        _ => &mut form.nickname,
    }
}

fn rooms_key(browser: &mut RoomBrowser, key: KeyEvent, quit: &mut bool) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => *quit = true,
        KeyCode::Up => browser.selected = browser.selected.saturating_sub(1),
        KeyCode::Down => {
            browser.selected = (browser.selected + 1).min(browser.rooms.len().saturating_sub(1));
        }
        KeyCode::Char('r') => return Some(Action::RefreshRooms),
        KeyCode::Char('n') => return Some(Action::OpenCreateRoom),
        KeyCode::Char('4') => return Some(Action::EnterMatchmaking { variant: "yonma" }),
        KeyCode::Char('3') => return Some(Action::EnterMatchmaking { variant: "sanma" }),
        KeyCode::Enter | KeyCode::Char('j') => {
            if let Some(room) = browser.rooms.get(browser.selected) {
                return Some(Action::JoinRoom {
                    id: room.id.clone(),
                    version: room.version,
                });
            }
        }
        _ => {}
    }
    None
}

fn matchmaking_key(_ticket: &MatchmakingTicketView, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('r') => Some(Action::RefreshMatchmaking),
        KeyCode::Esc | KeyCode::Char('c') => Some(Action::CancelMatchmaking),
        _ => None,
    }
}

fn create_room_key(form: &mut CreateRoomForm, key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        return Some(match form.create_payload() {
            Ok(payload) => Action::CreateRoom { payload },
            Err(error) => return Some(Action::ShowStatus(error.to_string())),
        });
    }
    match key.code {
        KeyCode::Esc => return Some(Action::BackToRooms),
        KeyCode::Tab | KeyCode::Down => form.next_field(),
        KeyCode::BackTab | KeyCode::Up => form.previous_field(),
        KeyCode::Char('[') => form.change_page(-1),
        KeyCode::Char(']') => form.change_page(1),
        KeyCode::Left => form.change_active(-1),
        KeyCode::Right => form.change_active(1),
        KeyCode::Backspace => form.backspace(),
        KeyCode::Char(' ') if !form.active_accepts_text() => form.change_active(1),
        KeyCode::Enter if !form.active_accepts_text() => form.change_active(1),
        KeyCode::Char(character) => form.push_character(character),
        _ => {}
    }
    None
}

fn room_key(screen: &mut RoomScreen, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(' ') => return Some(Action::Ready),
        KeyCode::Char('s') => return Some(Action::StartRoom),
        KeyCode::Char('r') => return Some(Action::RefreshRoom),
        KeyCode::Char('l') | KeyCode::Esc => return Some(Action::LeaveRoom),
        KeyCode::Up => screen.rules_scroll = screen.rules_scroll.saturating_sub(1),
        KeyCode::Down => screen.rules_scroll = screen.rules_scroll.saturating_add(1),
        _ => {}
    }
    None
}

fn game_key(game: &mut GameScreen, key: KeyEvent, quit: &mut bool) -> Option<Action> {
    match key.code {
        KeyCode::Char('b') | KeyCode::Esc if game.view.result.is_some() => {
            return Some(Action::ReturnToRoom);
        }
        KeyCode::Char('q') => *quit = true,
        KeyCode::Left => game.selected_tile = game.selected_tile.saturating_sub(1),
        KeyCode::Right => {
            let count = own_tiles(&game.view).map_or(0, Vec::len);
            game.selected_tile = (game.selected_tile + 1).min(count.saturating_sub(1));
        }
        KeyCode::Char(' ') => toggle_selected_tile_mark(game),
        KeyCode::Char('d') | KeyCode::Enter => {
            return selected_tile_command(game, "riichi.discard");
        }
        KeyCode::Char('r') if !game.view.turn_actions.riichi_discard_tile_ids.is_empty() => {
            let tile = own_tiles(&game.view).and_then(|tiles| tiles.get(game.selected_tile))?;
            if !game
                .view
                .turn_actions
                .riichi_discard_tile_ids
                .contains(&tile.id)
            {
                return Some(Action::ShowStatus("所选牌不能立直".to_owned()));
            }
            return selected_tile_command(game, "riichi.riichi_discard");
        }
        KeyCode::Char('t') if game.view.turn_actions.can_tsumo => {
            return Some(Action::GameCommand {
                name: "riichi.tsumo",
                payload: None,
            });
        }
        KeyCode::Char('s') if !game.view.available_reactions.is_empty() => {
            return Some(Action::GameCommand {
                name: "riichi.pass",
                payload: None,
            });
        }
        KeyCode::Char('h') if has_reaction(&game.view, ReactionKind::Ron) => {
            return Some(Action::GameCommand {
                name: "riichi.ron",
                payload: None,
            });
        }
        KeyCode::Char('c') if has_reaction(&game.view, ReactionKind::Chi) => {
            return Some(reaction_tiles_command(
                game,
                ReactionKind::Chi,
                "riichi.chi",
                2,
                "吃牌需标记 2 张手牌",
            ));
        }
        KeyCode::Char('p') | KeyCode::Char('o') if has_reaction(&game.view, ReactionKind::Pon) => {
            return Some(reaction_tiles_command(
                game,
                ReactionKind::Pon,
                "riichi.pon",
                2,
                "碰牌需标记 2 张手牌",
            ));
        }
        KeyCode::Char('k') => {
            if matches!(game.view.phase, MatchPhase::AwaitingResponses { .. }) {
                if !has_reaction(&game.view, ReactionKind::OpenKan) {
                    return Some(Action::ShowStatus("当前不能杠".to_owned()));
                }
                return Some(reaction_tiles_command(
                    game,
                    ReactionKind::OpenKan,
                    "riichi.open_kan",
                    3,
                    "明杠需标记 3 张手牌",
                ));
            }
            return Some(concealed_kan_command(game));
        }
        KeyCode::Char('a') if !game.view.turn_actions.added_kan_options.is_empty() => {
            return Some(added_kan_command(game));
        }
        KeyCode::Char('9') if game.view.turn_actions.can_nine_terminals => {
            return Some(Action::GameCommand {
                name: "riichi.nine_terminals",
                payload: None,
            });
        }
        KeyCode::Char('x') => return Some(Action::RefreshGame),
        _ => {}
    }
    None
}

fn selected_tile_command(game: &GameScreen, name: &'static str) -> Option<Action> {
    let tile = own_tiles(&game.view)?.get(game.selected_tile)?;
    Some(Action::GameCommand {
        name,
        payload: Some(json!({"tile_id": tile.id})),
    })
}

fn toggle_selected_tile_mark(game: &mut GameScreen) {
    let Some(tile) = own_tiles(&game.view).and_then(|tiles| tiles.get(game.selected_tile)) else {
        return;
    };
    if let Some(position) = game
        .marked_tile_ids
        .iter()
        .position(|tile_id| *tile_id == tile.id)
    {
        game.marked_tile_ids.remove(position);
    } else {
        game.marked_tile_ids.push(tile.id);
    }
}

fn marked_tiles_command(
    game: &GameScreen,
    name: &'static str,
    expected_count: usize,
    invalid_selection: &'static str,
) -> Action {
    if game.marked_tile_ids.len() != expected_count {
        return Action::ShowStatus(invalid_selection.to_owned());
    }
    if let Some(kind) = reaction_kind_for_command(name)
        && !reaction_selection_allowed(&game.view, kind, &game.marked_tile_ids)
    {
        return Action::ShowStatus("所选牌不能执行该操作".to_owned());
    }
    Action::GameCommand {
        name,
        payload: Some(json!({"tile_ids": game.marked_tile_ids})),
    }
}

fn reaction_tiles_command(
    game: &GameScreen,
    kind: ReactionKind,
    name: &'static str,
    expected_count: usize,
    invalid_selection: &'static str,
) -> Action {
    let candidates = reaction_tile_candidates(&game.view, kind);
    if candidates.len() == 1 {
        return Action::GameCommand {
            name,
            payload: Some(json!({"tile_ids": candidates[0]})),
        };
    }
    marked_tiles_command(game, name, expected_count, invalid_selection)
}

fn added_kan_command(game: &GameScreen) -> Action {
    let Some(tile) = own_tiles(&game.view).and_then(|tiles| tiles.get(game.selected_tile)) else {
        return Action::ShowStatus("请先选择要加杠的牌".to_owned());
    };
    let Some(option) = game
        .view
        .turn_actions
        .added_kan_options
        .iter()
        .find(|option| option.tile_id == tile.id)
    else {
        return Action::ShowStatus("所选牌不能加杠".to_owned());
    };
    Action::GameCommand {
        name: "riichi.added_kan",
        payload: Some(json!({"meld_id": option.meld_id, "tile_id": tile.id})),
    }
}

fn concealed_kan_command(game: &GameScreen) -> Action {
    let selected = own_tiles(&game.view)
        .and_then(|tiles| tiles.get(game.selected_tile))
        .map(|tile| tile.id);
    let candidates = &game.view.turn_actions.concealed_kan_tile_ids;
    let candidate = selected
        .and_then(|tile_id| {
            candidates
                .iter()
                .find(|candidate| candidate.contains(&tile_id))
        })
        .or_else(|| (candidates.len() == 1).then(|| &candidates[0]));
    let Some(candidate) = candidate else {
        return Action::ShowStatus("请选择要暗杠的牌".to_owned());
    };
    Action::GameCommand {
        name: "riichi.concealed_kan",
        payload: Some(json!({"tile_ids": candidate})),
    }
}

#[derive(Clone, Copy)]
enum ReactionKind {
    Ron,
    Chi,
    Pon,
    OpenKan,
}

fn has_reaction(view: &MatchView, expected: ReactionKind) -> bool {
    view.available_reactions.iter().any(|reaction| {
        matches!(
            (expected, reaction),
            (ReactionKind::Ron, ReactionOptionView::Ron)
                | (ReactionKind::Chi, ReactionOptionView::Chi { .. })
                | (ReactionKind::Pon, ReactionOptionView::Pon { .. })
                | (ReactionKind::OpenKan, ReactionOptionView::OpenKan { .. })
        )
    })
}

fn reaction_kind_for_command(name: &str) -> Option<ReactionKind> {
    match name {
        "riichi.chi" => Some(ReactionKind::Chi),
        "riichi.pon" => Some(ReactionKind::Pon),
        "riichi.open_kan" => Some(ReactionKind::OpenKan),
        _ => None,
    }
}

fn reaction_selection_allowed(view: &MatchView, expected: ReactionKind, selected: &[u16]) -> bool {
    reaction_tile_candidates(view, expected)
        .into_iter()
        .any(|candidate| {
            candidate.len() == selected.len()
                && candidate.iter().all(|tile_id| selected.contains(tile_id))
        })
}

fn reaction_tile_candidates(view: &MatchView, expected: ReactionKind) -> Vec<&[u16]> {
    view.available_reactions
        .iter()
        .filter_map(|reaction| match (expected, reaction) {
            (ReactionKind::Chi, ReactionOptionView::Chi { tile_ids })
            | (ReactionKind::Pon, ReactionOptionView::Pon { tile_ids }) => {
                Some(tile_ids.as_slice())
            }
            (ReactionKind::OpenKan, ReactionOptionView::OpenKan { tile_ids }) => {
                Some(tile_ids.as_slice())
            }
            _ => None,
        })
        .collect()
}

fn own_tiles(view: &MatchView) -> Option<&Vec<crate::model::TileView>> {
    view.players
        .iter()
        .find(|player| player.seat == view.observer_seat)?
        .concealed_tiles
        .as_ref()
}

fn clamp_selection(selected: usize, view: &MatchView) -> usize {
    selected.min(own_tiles(view).map_or(0, Vec::len).saturating_sub(1))
}

fn retained_marks(marked: &[u16], view: &MatchView, old_hand: u32) -> Vec<u16> {
    if old_hand != view.hand_index {
        return Vec::new();
    }
    let Some(tiles) = own_tiles(view) else {
        return Vec::new();
    };
    marked
        .iter()
        .copied()
        .filter(|marked_id| tiles.iter().any(|tile| tile.id == *marked_id))
        .collect()
}

fn command_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("c{millis:016x}")
}

fn command_status(name: &str) -> &'static str {
    match name {
        "riichi.discard" => "已打牌",
        "riichi.riichi_discard" => "立直",
        "riichi.tsumo" => "自摸",
        "riichi.pass" => "已过",
        "riichi.ron" => "荣和",
        "riichi.chi" => "已吃牌",
        "riichi.pon" => "已碰牌",
        "riichi.open_kan" | "riichi.concealed_kan" | "riichi.added_kan" => "已杠牌",
        "riichi.nine_terminals" => "九种九牌",
        _ => "操作成功",
    }
}

#[cfg(test)]
mod tests {
    use super::command_status;

    #[test]
    fn protocol_command_names_do_not_leak_into_game_copy() {
        assert_eq!(command_status("riichi.discard"), "已打牌");
        assert_eq!(command_status("riichi.concealed_kan"), "已杠牌");
    }
}
