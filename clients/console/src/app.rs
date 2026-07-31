use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};

use crate::api::ApiClient;
use crate::model::{
    MatchPhase, MatchView, MatchmakingTicketView, ReactionOptionView, RoomView, UserView,
};

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
pub struct CreateRoomForm {
    pub active_field: usize,
    pub name: String,
    pub variant: String,
    pub initial_points: String,
    pub noten_payment: String,
    pub head_bump: bool,
    pub tobi: bool,
}

#[derive(Debug)]
pub struct GameScreen {
    pub view: MatchView,
    pub selected_tile: usize,
    pub marked_tile_ids: Vec<u16>,
}

#[derive(Debug)]
pub enum Screen {
    Auth(AuthForm),
    Rooms(RoomBrowser),
    CreateRoom(CreateRoomForm),
    Matchmaking(MatchmakingTicketView),
    Room(RoomView),
    Game(GameScreen),
}

pub struct App {
    pub screen: Screen,
    pub user: Option<UserView>,
    pub status: String,
    pub quit: bool,
    api: ApiClient,
    last_poll: Instant,
}

enum Action {
    ShowStatus(&'static str),
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
        name: String,
        variant: String,
        initial_points: String,
        noten_payment: String,
        head_bump: bool,
        tobi: bool,
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
            last_poll: Instant::now(),
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
        if self.last_poll.elapsed() < POLL_INTERVAL {
            return false;
        }
        self.last_poll = Instant::now();
        let action = match self.screen {
            Screen::Room(_) => Some(Action::RefreshRoom),
            Screen::Matchmaking(_) => Some(Action::RefreshMatchmaking),
            Screen::Game(_) => Some(Action::RefreshGame),
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
            Screen::Room(room) => room_key(room, key),
            Screen::Game(game) => game_key(game, key, &mut self.quit),
        }
    }

    async fn perform(&mut self, action: Action) {
        let result = match action {
            Action::ShowStatus(message) => {
                self.status = message.to_owned();
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
                        self.load_rooms().await
                    }
                    Err(error) => Err(error),
                }
            }
            Action::RefreshRooms => self.load_rooms().await,
            Action::OpenCreateRoom => {
                self.screen = Screen::CreateRoom(CreateRoomForm {
                    active_field: 0,
                    name: "日麻房间".to_owned(),
                    variant: "yonma".to_owned(),
                    initial_points: "25000".to_owned(),
                    noten_payment: "3000".to_owned(),
                    head_bump: false,
                    tobi: true,
                });
                Ok(())
            }
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
            Action::CreateRoom {
                name,
                variant,
                initial_points,
                noten_payment,
                head_bump,
                tobi,
            } => {
                match (
                    parse_number(&initial_points, "初始点数"),
                    parse_number(&noten_payment, "流局罚点"),
                ) {
                    (Ok(initial_points), Ok(noten_payment)) => self
                        .api
                        .create_room(
                            &name,
                            &variant,
                            initial_points,
                            tobi,
                            noten_payment,
                            head_bump,
                        )
                        .await
                        .map(|room| {
                            self.status = "房间已创建".to_owned();
                            self.screen = Screen::Room(room);
                        }),
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            Action::JoinRoom { id, version } => {
                self.api.join_room(&id, version).await.map(|room| {
                    self.status = "已加入房间".to_owned();
                    self.screen = Screen::Room(room);
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
                        self.screen = Screen::Room(room);
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

    async fn load_rooms(&mut self) -> Result<(), crate::model::ApiFailure> {
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
        let room = self.api.room(&current.id).await?;
        if let Some(match_id) = &room.active_match_id {
            let view = self.api.match_view(match_id).await?;
            self.screen = Screen::Game(GameScreen {
                view,
                selected_tile: 0,
                marked_tile_ids: Vec::new(),
            });
        } else {
            self.screen = Screen::Room(room);
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
        if let Some(match_id) = &ticket.match_id {
            let view = self.api.match_view(match_id).await?;
            self.status = "匹配成功，对局开始".to_owned();
            self.screen = Screen::Game(GameScreen {
                view,
                selected_tile: 0,
                marked_tile_ids: Vec::new(),
            });
        } else {
            self.status = "正在匹配".to_owned();
            self.screen = Screen::Matchmaking(ticket);
        }
        Ok(())
    }

    async fn toggle_ready(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(room) = &self.screen else {
            return Ok(());
        };
        let Some(user) = &self.user else {
            return Ok(());
        };
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
        self.screen = Screen::Room(room);
        Ok(())
    }

    async fn start_room(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(room) = &self.screen else {
            return Ok(());
        };
        let started = self.api.start_room(&room.id, room.version).await?;
        let view = self.api.match_view(&started.match_id).await?;
        self.status = "对局开始".to_owned();
        self.screen = Screen::Game(GameScreen {
            view,
            selected_tile: 0,
            marked_tile_ids: Vec::new(),
        });
        Ok(())
    }

    async fn leave_room(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Room(room) = &self.screen else {
            return Ok(());
        };
        self.api.leave_room(&room.id, room.version).await?;
        self.load_rooms().await
    }

    async fn refresh_game(&mut self) -> Result<(), crate::model::ApiFailure> {
        let Screen::Game(game) = &self.screen else {
            return Ok(());
        };
        let old_hand = game.view.hand_index;
        let view = self.api.match_view(&game.view.id).await?;
        let selected = clamp_selection(game.selected_tile, &view);
        let marked_tile_ids = retained_marks(&game.marked_tile_ids, &view, old_hand);
        self.screen = Screen::Game(GameScreen {
            view,
            selected_tile: selected,
            marked_tile_ids,
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
        let mut view = self
            .api
            .game_command(&game.view.id, game.view.version, name, payload)
            .await?;
        let selected = clamp_selection(game.selected_tile, &view);
        if view.result.is_some() {
            self.status = "整场结束".to_owned();
        } else {
            self.status = command_status(name).to_owned();
        }
        // Keep the latest observer projection only.
        view.players.shrink_to_fit();
        self.screen = Screen::Game(GameScreen {
            view,
            selected_tile: selected,
            marked_tile_ids: Vec::new(),
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
    match key.code {
        KeyCode::Esc => return Some(Action::BackToRooms),
        KeyCode::Tab | KeyCode::Down => form.active_field = (form.active_field + 1) % 6,
        KeyCode::BackTab | KeyCode::Up => {
            form.active_field = (form.active_field + 5) % 6;
        }
        KeyCode::F(3) => toggle_create_option(form, 3),
        KeyCode::F(4) => toggle_create_option(form, 4),
        KeyCode::F(5) => toggle_create_option(form, 5),
        KeyCode::Left | KeyCode::Right if form.active_field >= 3 => {
            toggle_create_option(form, form.active_field);
        }
        KeyCode::Backspace => {
            if let Some(text) = active_create_text(form) {
                text.pop();
            }
        }
        KeyCode::Char(' ') if form.active_field >= 3 => {
            toggle_create_option(form, form.active_field);
        }
        KeyCode::Char(character) => {
            if let Some(text) = active_create_text(form) {
                text.push(character);
            }
        }
        KeyCode::Enter => {
            return Some(Action::CreateRoom {
                name: form.name.clone(),
                variant: form.variant.clone(),
                initial_points: form.initial_points.clone(),
                noten_payment: form.noten_payment.clone(),
                head_bump: form.head_bump,
                tobi: form.tobi,
            });
        }
        _ => {}
    }
    None
}

fn active_create_text(form: &mut CreateRoomForm) -> Option<&mut String> {
    match form.active_field {
        0 => Some(&mut form.name),
        1 => Some(&mut form.initial_points),
        2 => Some(&mut form.noten_payment),
        _ => None,
    }
}

fn toggle_create_option(form: &mut CreateRoomForm, field: usize) {
    match field {
        3 => {
            form.variant = if form.variant == "yonma" {
                "sanma".to_owned()
            } else {
                "yonma".to_owned()
            };
        }
        4 => form.head_bump = !form.head_bump,
        5 => form.tobi = !form.tobi,
        _ => {}
    }
}

fn room_key(room: &RoomView, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(' ') => Some(Action::Ready),
        KeyCode::Char('s') => Some(Action::StartRoom),
        KeyCode::Char('r') => Some(Action::RefreshRoom),
        KeyCode::Char('l') | KeyCode::Esc => Some(Action::LeaveRoom),
        _ => {
            let _ = room;
            None
        }
    }
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
                return Some(Action::ShowStatus("所选牌不能立直"));
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
                    return Some(Action::ShowStatus("当前不能杠"));
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
        return Action::ShowStatus(invalid_selection);
    }
    if let Some(kind) = reaction_kind_for_command(name)
        && !reaction_selection_allowed(&game.view, kind, &game.marked_tile_ids)
    {
        return Action::ShowStatus("所选牌不能执行该操作");
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
        return Action::ShowStatus("请先选择要加杠的牌");
    };
    let Some(option) = game
        .view
        .turn_actions
        .added_kan_options
        .iter()
        .find(|option| option.tile_id == tile.id)
    else {
        return Action::ShowStatus("所选牌不能加杠");
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
        return Action::ShowStatus("请选择要暗杠的牌");
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

fn parse_number(value: &str, label: &str) -> Result<u32, crate::model::ApiFailure> {
    value.parse().map_err(|_| crate::model::ApiFailure {
        code: "client.invalid_input".to_owned(),
        message: format!("{label}必须是非负整数"),
    })
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{CreateRoomForm, command_status, create_room_key};

    #[test]
    fn protocol_command_names_do_not_leak_into_game_copy() {
        assert_eq!(command_status("riichi.discard"), "已打牌");
        assert_eq!(command_status("riichi.concealed_kan"), "已杠牌");
    }

    #[test]
    fn room_rule_shortcuts_do_not_consume_text_characters() {
        let mut form = CreateRoomForm {
            active_field: 0,
            name: String::new(),
            variant: "yonma".to_owned(),
            initial_points: "25000".to_owned(),
            noten_payment: "3000".to_owned(),
            head_bump: false,
            tobi: true,
        };

        create_room_key(
            &mut form,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
        );
        assert_eq!(form.name, "v");
        assert_eq!(form.variant, "yonma");

        form.active_field = 3;
        create_room_key(
            &mut form,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(form.variant, "sanma");
    }
}
