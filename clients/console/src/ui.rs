use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use crate::app::{App, AuthMode, CreateRoomForm, GameScreen, RoomBrowser, Screen};
use crate::model::{
    MatchPlayerView, MatchResultView, MatchView, MatchmakingTicketView, ReactionOptionView,
    RoomView, TileView,
};

const FELT: Color = Color::Rgb(18, 82, 64);
const IVORY: Color = Color::Rgb(245, 238, 214);
const GOLD: Color = Color::Rgb(230, 184, 82);
const INK: Color = Color::Rgb(10, 35, 31);
const MUTED: Color = Color::Rgb(153, 166, 160);
const MIN_WIDTH: u16 = 76;
const MIN_HEIGHT: u16 = 22;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    if frame.area().width < MIN_WIDTH || frame.area().height < MIN_HEIGHT {
        render_too_small(frame, frame.area());
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());
    render_header(frame, chunks[0], app);
    match &app.screen {
        Screen::Auth(form) => render_auth(frame, chunks[1], form),
        Screen::Rooms(browser) => render_rooms(frame, chunks[1], browser),
        Screen::CreateRoom(form) => render_create(frame, chunks[1], form),
        Screen::Matchmaking(ticket) => render_matchmaking(frame, chunks[1], ticket),
        Screen::Room(room) => render_room(frame, chunks[1], room, app),
        Screen::Game(game) => render_game(frame, chunks[1], game),
    }
    render_footer(frame, chunks[2], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);
    let screen = match &app.screen {
        Screen::Auth(_) => "登录",
        Screen::Rooms(_) => "大厅",
        Screen::CreateRoom(_) => "创建房间",
        Screen::Matchmaking(_) => "段位匹配",
        Screen::Room(room) => room.name.as_str(),
        Screen::Game(_) => "对局",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " 麻将 ",
                Style::default()
                    .fg(INK)
                    .bg(GOLD)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {screen}"), Style::default().fg(IVORY).bg(INK)),
        ]))
        .style(Style::default().bg(INK)),
        columns[0],
    );
    let user = app
        .user
        .as_ref()
        .map_or("", |user| user.profile.nickname.as_str());
    frame.render_widget(
        Paragraph::new(format!("{user} "))
            .alignment(Alignment::Right)
            .style(Style::default().fg(MUTED).bg(INK)),
        columns[1],
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let help = help_text(app);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" {}", app.status),
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                format!(" {help}"),
                Style::default().fg(Color::Black).bg(GOLD),
            )),
        ])
        .style(Style::default().bg(Color::Rgb(32, 39, 37))),
        area,
    );
}

fn help_text(app: &App) -> String {
    match &app.screen {
        Screen::Auth(form) => format!(
            "Tab 切换  Enter {}  F2 {}  Esc 退出",
            if form.mode == AuthMode::Login {
                "登录"
            } else {
                "注册"
            },
            if form.mode == AuthMode::Login {
                "注册"
            } else {
                "登录"
            }
        ),
        Screen::Rooms(_) => {
            "↑↓ 选择  Enter 加入  4 四麻匹配  3 三麻匹配  n 建房  r 刷新  q 退出".to_owned()
        }
        Screen::CreateRoom(_) => "Tab 切换  ←→ / Space 修改  Enter 创建  Esc 返回".to_owned(),
        Screen::Matchmaking(_) => "Esc 取消匹配".to_owned(),
        Screen::Room(room) => {
            let ready = current_user_ready(room, app);
            format!(
                "Space {}  s 开始  r 刷新  Esc 离开",
                if ready { "取消准备" } else { "准备" }
            )
        }
        Screen::Game(game) if game.view.result.is_some() => "Esc 返回房间  q 退出".to_owned(),
        Screen::Game(game) => game_help(&game.view),
    }
}

fn game_help(view: &MatchView) -> String {
    if !view.available_reactions.is_empty() {
        let mut actions = Vec::new();
        if has_reaction(view, "ron") {
            actions.push("h 荣和");
        }
        if has_reaction(view, "pon") {
            actions.push("p 碰");
        }
        if has_reaction(view, "open_kan") {
            actions.push("k 杠");
        }
        if has_reaction(view, "chi") {
            actions.push("c 吃");
        }
        actions.push("s 过");
        if view
            .available_reactions
            .iter()
            .filter(|reaction| !matches!(reaction, ReactionOptionView::Ron))
            .count()
            > 1
        {
            actions.insert(0, "Space 选牌");
        }
        return actions.join("  ");
    }
    let own_turn = matches!(
        view.phase,
        crate::model::MatchPhase::AwaitingTurnAction { seat }
            | crate::model::MatchPhase::AwaitingDiscard { seat }
            if seat == view.observer_seat
    );
    if own_turn {
        let mut actions = vec!["←→ 选牌", "Enter 打牌"];
        if !view.turn_actions.riichi_discard_tile_ids.is_empty() {
            actions.push("r 立直");
        }
        if view.turn_actions.can_tsumo {
            actions.push("t 自摸");
        }
        if !view.turn_actions.concealed_kan_tile_ids.is_empty() {
            actions.push("k 暗杠");
        }
        if !view.turn_actions.added_kan_options.is_empty() {
            actions.push("a 加杠");
        }
        if view.turn_actions.can_nine_terminals {
            actions.push("9 九种九牌");
        }
        actions.join("  ")
    } else {
        "等待其他玩家  x 同步  q 退出".to_owned()
    }
}

fn has_reaction(view: &MatchView, kind: &str) -> bool {
    view.available_reactions.iter().any(|reaction| {
        matches!(
            (kind, reaction),
            ("ron", ReactionOptionView::Ron)
                | ("chi", ReactionOptionView::Chi { .. })
                | ("pon", ReactionOptionView::Pon { .. })
                | ("open_kan", ReactionOptionView::OpenKan { .. })
        )
    })
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("终端窗口过小"),
            Line::from(""),
            Line::from(format!(
                "至少需要 {MIN_WIDTH}×{MIN_HEIGHT}，当前 {}×{}",
                area.width, area.height
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().fg(IVORY).bg(INK))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD)),
        ),
        centered(area, area.width.min(48), area.height.min(7)),
    );
}

fn render_auth(frame: &mut Frame<'_>, area: Rect, form: &crate::app::AuthForm) {
    let width = area.width.min(52);
    let height = if form.mode == AuthMode::Register {
        12
    } else {
        9
    };
    let dialog = centered(area, width, height);
    frame.render_widget(Clear, dialog);
    let title = match form.mode {
        AuthMode::Login => "登录",
        AuthMode::Register => "注册",
    };
    let masked_password = "•".repeat(form.password.chars().count());
    let mut lines = vec![
        Line::from(field_line(
            "登录名",
            &form.login_name,
            form.active_field == 0,
        )),
        Line::from(""),
        Line::from(field_line("密码", &masked_password, form.active_field == 1)),
    ];
    if form.mode == AuthMode::Register {
        lines.push(Line::from(""));
        lines.push(Line::from(field_line(
            "昵称",
            &form.nickname,
            form.active_field == 2,
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD))
                .title(format!(" {title} ")),
        ),
        dialog,
    );
}

fn render_rooms(frame: &mut Frame<'_>, area: Rect, browser: &RoomBrowser) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);
    let items = if browser.rooms.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  暂无公开房间",
            Style::default().fg(MUTED),
        )]))]
    } else {
        browser
            .rooms
            .iter()
            .enumerate()
            .map(|(index, room)| {
                ListItem::new(format!(
                    "  {}  {}/{}  {}  {}",
                    room.name,
                    room.members.len(),
                    seat_count(room),
                    variant_label(room),
                    lifecycle_label(&room.lifecycle)
                ))
                .style(if index == browser.selected {
                    Style::default().fg(Color::Black).bg(GOLD)
                } else {
                    Style::default()
                })
            })
            .collect()
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" 公开房间 ({}) ", browser.rooms.len())),
        ),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "4  四人东南战",
                Style::default().fg(INK).bg(GOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "3  三人东南战",
                Style::default().fg(INK).bg(GOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("n  创建房间", Style::default().fg(IVORY))),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" 快速开始 "),
        ),
        columns[1],
    );
}

fn render_matchmaking(frame: &mut Frame<'_>, area: Rect, ticket: &MatchmakingTicketView) {
    let dialog = centered(area, area.width.min(46), 8);
    frame.render_widget(Clear, dialog);
    let variant = if ticket.rule_set_id == "riichi/yonma" {
        "四人东南战"
    } else {
        "三人东南战"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                variant,
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(match ticket.status.as_str() {
                "waiting" => "匹配中",
                "matched" => "已匹配",
                "cancelled" => "已取消",
                _ => "匹配状态已更新",
            }),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD))
                .title(" 段位匹配 "),
        ),
        dialog,
    );
}

fn render_create(frame: &mut Frame<'_>, area: Rect, form: &CreateRoomForm) {
    let dialog = centered(area, area.width.min(62), 15);
    frame.render_widget(Clear, dialog);
    let lines = vec![
        Line::from(field_line("房间名", &form.name, form.active_field == 0)),
        Line::from(""),
        Line::from(field_line(
            "初始点数",
            &form.initial_points,
            form.active_field == 1,
        )),
        Line::from(""),
        Line::from(field_line(
            "流局罚点",
            &form.noten_payment,
            form.active_field == 2,
        )),
        Line::from(""),
        create_option_line(form),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD))
                .title(" 创建房间 "),
        ),
        dialog,
    );
}

fn create_option_line(form: &CreateRoomForm) -> Line<'static> {
    let option = |label: String, active: bool| {
        Span::styled(
            label,
            if active {
                Style::default()
                    .fg(Color::Black)
                    .bg(GOLD)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        )
    };
    Line::from(vec![
        Span::raw("人数  "),
        option(
            if form.variant == "yonma" {
                "[四麻]".to_owned()
            } else {
                "[三麻]".to_owned()
            },
            form.active_field == 3,
        ),
        Span::raw("    荣和  "),
        option(
            if form.head_bump {
                "[头跳]".to_owned()
            } else {
                "[多家和]".to_owned()
            },
            form.active_field == 4,
        ),
        Span::raw("    击飞  "),
        option(
            if form.tobi {
                "[有]".to_owned()
            } else {
                "[无]".to_owned()
            },
            form.active_field == 5,
        ),
    ])
}

fn render_room(frame: &mut Frame<'_>, area: Rect, room: &RoomView, _app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let members = room
        .members
        .iter()
        .map(|member| {
            let owner = if member.user_id == room.owner_user_id {
                " 房主"
            } else {
                ""
            };
            ListItem::new(format!(
                " {}家  {:<12} {}{}",
                wind_for_seat(member.seat),
                member.nickname,
                if member.ready {
                    "已准备"
                } else {
                    "等待中"
                },
                owner
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(members).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(
                    " 成员 {}/{} ",
                    room.members.len(),
                    seat_count(room)
                )),
        ),
        columns[0],
    );
    let config = &room.rule_snapshot["config"];
    let lines = vec![
        Line::from(format!("{}东南战", variant_label(room))),
        Line::from(""),
        Line::from(format!(
            "持有点  {}",
            config["match_rules"]["initial_points"]
        )),
        Line::from(format!(
            "流局罚点 {}",
            config["settlement"]["noten_payment"]
        )),
        Line::from(format!(
            "荣和方式 {}",
            if config["settlement"]["ron_resolution"] == "head_bump" {
                "头跳"
            } else {
                "多家和"
            }
        )),
        Line::from(format!(
            "击飞     {}",
            if config["match_rules"]["tobi"] == true {
                "有"
            } else {
                "无"
            }
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" 规则 "),
        ),
        columns[1],
    );
}

fn render_game(frame: &mut Frame<'_>, area: Rect, game: &GameScreen) {
    let view = &game.view;
    frame.render_widget(Block::default().style(Style::default().bg(FELT)), area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(45),
            Constraint::Percentage(30),
        ])
        .split(area);
    let relative = relative_seats(view);
    if let Some(top) = player(view, relative[2]) {
        render_player(frame, rows[0], top, false, 0, &[], view.progress.dealer);
    }
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(rows[1]);
    if let Some(left) = player(view, relative[3]) {
        render_player(frame, middle[0], left, false, 0, &[], view.progress.dealer);
    }
    render_center(frame, middle[1], view);
    if let Some(right) = player(view, relative[1]) {
        render_player(frame, middle[2], right, false, 0, &[], view.progress.dealer);
    }
    if let Some(bottom) = player(view, relative[0]) {
        render_player(
            frame,
            rows[2],
            bottom,
            true,
            game.selected_tile,
            &game.marked_tile_ids,
            view.progress.dealer,
        );
    }
    if let Some(result) = &view.result {
        render_result(frame, area, result);
    }
}

fn render_player(
    frame: &mut Frame<'_>,
    area: Rect,
    player: &MatchPlayerView,
    own: bool,
    selected: usize,
    marked: &[u16],
    dealer: u8,
) {
    let mut state = String::new();
    if player.seat == dealer {
        state.push_str(" · 亲");
    }
    if player.riichi_status == "established" {
        state.push_str(" · 立直");
    }
    let title = format!(
        "{}家 · {} · {}点{}",
        wind_for_seat(player.seat),
        player.nickname,
        player.points,
        state
    );
    let mut lines = Vec::new();
    let discards = player
        .discards
        .iter()
        .map(|discard| {
            if discard.riichi_declared {
                format!("{}↔", tile_label(&discard.tile.code))
            } else if discard.claimed_by.is_some() {
                format!("({})", tile_label(&discard.tile.code))
            } else if discard.tsumogiri {
                format!("{}·", tile_label(&discard.tile.code))
            } else {
                tile_label(&discard.tile.code)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(Line::from(format!("河：{discards}")));
    if !player.melds.is_empty() {
        lines.push(Line::from(format!(
            "副露：{}",
            player
                .melds
                .iter()
                .map(|meld| format!(
                    "{}[{}]",
                    meld_label(&meld.kind),
                    meld.tiles
                        .iter()
                        .map(|tile| tile_label(&tile.code))
                        .collect::<Vec<_>>()
                        .join(" ")
                ))
                .collect::<Vec<_>>()
                .join("  ")
        )));
    }
    if own {
        let hand = player
            .concealed_tiles
            .as_ref()
            .map(|tiles| tile_line(tiles, selected, marked, player.drawn_tile_id))
            .unwrap_or_default();
        lines.push(Line::from(""));
        lines.push(hand);
    } else {
        lines.push(Line::from(format!(
            "手牌：{} 张",
            player.concealed_tile_count
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(Color::White).bg(FELT))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if own { GOLD } else { Color::Gray }))
                    .title(title),
            ),
        area,
    );
}

fn render_center(frame: &mut Frame<'_>, area: Rect, view: &MatchView) {
    let dora = view
        .dora_indicators
        .iter()
        .map(|tile| tile_label(&tile.code))
        .collect::<Vec<_>>()
        .join(" ");
    let phase = match view.phase {
        crate::model::MatchPhase::AwaitingTurnAction { seat } => {
            format!("{}家摸牌", wind_for_seat(seat))
        }
        crate::model::MatchPhase::AwaitingDiscard { seat } => {
            format!("{}家打牌", wind_for_seat(seat))
        }
        crate::model::MatchPhase::AwaitingResponses { trigger_seat } => {
            if view.available_reactions.is_empty() {
                format!("等待其他玩家 · {}家打牌", wind_for_seat(trigger_seat))
            } else {
                format!("可响应 · {}家打牌", wind_for_seat(trigger_seat))
            }
        }
        crate::model::MatchPhase::Ended { reason } => {
            format!("本局结束 · {}", end_reason_label(reason))
        }
    };
    let lines = vec![
        Line::from(Span::styled(
            format!(
                "{}{}局  {}本场",
                wind_label(&view.progress.round_wind),
                view.progress.round_number,
                view.progress.honba
            ),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "供托 {}　余牌 {}",
            view.progress.riichi_sticks, view.remaining_live_draws
        )),
        Line::from(format!("宝牌指示 {dora}")),
        Line::from(""),
        Line::from(phase),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default().fg(IVORY).bg(Color::Rgb(8, 55, 43)))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(GOLD)),
            ),
        area,
    );
}

fn render_result(frame: &mut Frame<'_>, area: Rect, result: &MatchResultView) {
    let dialog = centered(area, area.width.min(56), 10);
    frame.render_widget(Clear, dialog);
    let mut lines = vec![Line::from(Span::styled(
        match_end_reason_label(&result.end_reason),
        Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
    ))];
    for placement in &result.placements {
        lines.push(Line::from(format!(
            "{}位　{}家　{}点　{:+.1}",
            placement.rank,
            wind_for_seat(placement.seat),
            placement.points,
            f64::from(placement.score_tenths) / 10.0
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD))
                .title(" 对局结果 "),
        ),
        dialog,
    );
}

fn tile_line(
    tiles: &[TileView],
    selected: usize,
    marked: &[u16],
    drawn: Option<u16>,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, tile) in tiles.iter().enumerate() {
        let mut style = Style::default().fg(Color::Black).bg(IVORY);
        if tile.code.starts_with('0') {
            style = style.fg(Color::Red).add_modifier(Modifier::BOLD);
        }
        if index == selected {
            style = style.bg(GOLD).add_modifier(Modifier::BOLD);
        } else if marked.contains(&tile.id) {
            style = style
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD);
        }
        let marker = if marked.contains(&tile.id) {
            "●"
        } else if index == selected {
            "▴"
        } else if Some(tile.id) == drawn {
            "·"
        } else {
            " "
        };
        spans.push(Span::styled(
            format!("{}{marker}", tile_label(&tile.code)),
            style,
        ));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn field_line(label: &str, value: &str, active: bool) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("  {label}  "), Style::default().fg(MUTED)),
        Span::styled(
            format!("{value:<32}"),
            if active {
                Style::default()
                    .fg(Color::Black)
                    .bg(GOLD)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        ),
    ]
}

fn tile_label(code: &str) -> String {
    let Some(suit) = code.chars().nth(1) else {
        return code.to_owned();
    };
    let number = code.chars().next().unwrap_or('?');
    match (number, suit) {
        ('0', 'm') => "5萬".to_owned(),
        ('0', 'p') => "5筒".to_owned(),
        ('0', 's') => "5索".to_owned(),
        (number @ '1'..='9', 'm') => format!("{number}萬"),
        (number @ '1'..='9', 'p') => format!("{number}筒"),
        (number @ '1'..='9', 's') => format!("{number}索"),
        ('1', 'z') => "東".to_owned(),
        ('2', 'z') => "南".to_owned(),
        ('3', 'z') => "西".to_owned(),
        ('4', 'z') => "北".to_owned(),
        ('5', 'z') => "白".to_owned(),
        ('6', 'z') => "發".to_owned(),
        ('7', 'z') => "中".to_owned(),
        _ => code.to_owned(),
    }
}

fn relative_seats(view: &MatchView) -> [u8; 4] {
    let count = u8::try_from(view.players.len()).unwrap_or(4);
    let own = view.observer_seat;
    if count == 3 {
        [own, (own + 1) % 3, (own + 2) % 3, u8::MAX]
    } else {
        [own, (own + 1) % 4, (own + 2) % 4, (own + 3) % 4]
    }
}

fn player(view: &MatchView, seat: u8) -> Option<&MatchPlayerView> {
    view.players.iter().find(|player| player.seat == seat)
}

fn seat_count(room: &RoomView) -> u64 {
    if room.rule_snapshot["rule_set_id"] == "riichi/sanma" {
        3
    } else {
        4
    }
}

fn variant_label(room: &RoomView) -> &'static str {
    if seat_count(room) == 3 {
        "三麻"
    } else {
        "四麻"
    }
}

fn current_user_ready(room: &RoomView, app: &App) -> bool {
    let Some(user) = &app.user else {
        return false;
    };
    room.members
        .iter()
        .find(|member| member.user_id == user.id)
        .is_some_and(|member| member.ready)
}

fn lifecycle_label(lifecycle: &str) -> &'static str {
    match lifecycle {
        "waiting" => "等待中",
        "playing" => "进行中",
        "closed" => "已关闭",
        _ => "未知",
    }
}

fn meld_label(kind: &str) -> &'static str {
    match kind {
        "chi" => "吃",
        "pon" => "碰",
        "open_kan" => "明杠",
        "concealed_kan" => "暗杠",
        "added_kan" => "加杠",
        _ => "副露",
    }
}

const fn end_reason_label(reason: crate::model::EndReason) -> &'static str {
    match reason {
        crate::model::EndReason::ExhaustiveDraw => "荒牌流局",
        crate::model::EndReason::NineTerminals => "九种九牌",
        crate::model::EndReason::FourWinds => "四风连打",
        crate::model::EndReason::FourKans => "四杠散了",
        crate::model::EndReason::FourRiichi => "四家立直",
        crate::model::EndReason::Tsumo => "自摸",
        crate::model::EndReason::Ron => "荣和",
    }
}

fn match_end_reason_label(reason: &str) -> &'static str {
    match reason {
        "scheduled_end" => "规定局数结束",
        "tobi" => "击飞",
        "agari_yame" => "和牌止",
        _ => "对局结束",
    }
}

const fn wind_for_seat(seat: u8) -> &'static str {
    match seat {
        0 => "东",
        1 => "南",
        2 => "西",
        _ => "北",
    }
}

fn wind_label(wind: &str) -> &'static str {
    match wind {
        "east" => "东",
        "south" => "南",
        "west" => "西",
        _ => "北",
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{MIN_HEIGHT, MIN_WIDTH, render, tile_label};
    use crate::app::{App, GameScreen, Screen};
    use crate::model::MatchView;

    #[test]
    fn tile_codes_are_rendered_as_mahjong_faces() {
        assert_eq!(tile_label("1m"), "1萬");
        assert_eq!(tile_label("0p"), "5筒");
        assert_eq!(tile_label("6z"), "發");
        assert_eq!(tile_label("invalid"), "invalid");
    }

    #[test]
    fn auth_screen_keeps_controls_in_the_footer() {
        let app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        let backend = TestBackend::new(90, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("draw");

        let screen = screen_text(terminal.backend().buffer());
        assert!(screen.contains("麻将"));
        assert!(screen.contains("登录名"));
        assert!(screen.contains("Tab切换"));
    }

    #[test]
    fn small_terminal_has_a_single_clear_message() {
        let app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        let backend = TestBackend::new(MIN_WIDTH - 1, MIN_HEIGHT - 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("draw");

        assert!(screen_text(terminal.backend().buffer()).contains("终端窗口过小"));
    }

    #[test]
    fn game_screen_uses_localized_tiles_and_only_legal_actions() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        let view = serde_json::from_value::<MatchView>(serde_json::json!({
            "id": "match-1",
            "room_id": "room-1",
            "version": 1,
            "hand_index": 0,
            "observer_seat": 0,
            "progress": {
                "round_wind": "east",
                "round_number": 1,
                "dealer": 0,
                "honba": 0,
                "riichi_sticks": 0
            },
            "phase": {"kind": "awaiting_turn_action", "seat": 0},
            "remaining_live_draws": 69,
            "dora_indicators": [{"id": 90, "code": "1z"}],
            "players": [
                {
                    "seat": 0,
                    "nickname": "自家",
                    "points": 25000,
                    "concealed_tiles": [
                        {"id": 1, "code": "1m"},
                        {"id": 2, "code": "0p"}
                    ],
                    "concealed_tile_count": 2,
                    "drawn_tile_id": 2,
                    "melds": [],
                    "discards": [],
                    "riichi_status": "none"
                },
                {
                    "seat": 1,
                    "nickname": "下家",
                    "points": 25000,
                    "concealed_tiles": null,
                    "concealed_tile_count": 13,
                    "drawn_tile_id": null,
                    "melds": [],
                    "discards": [],
                    "riichi_status": "none"
                },
                {
                    "seat": 2,
                    "nickname": "对家",
                    "points": 25000,
                    "concealed_tiles": null,
                    "concealed_tile_count": 13,
                    "drawn_tile_id": null,
                    "melds": [],
                    "discards": [],
                    "riichi_status": "none"
                }
            ],
            "available_reactions": [],
            "turn_actions": {
                "can_tsumo": true,
                "riichi_discard_tile_ids": [],
                "concealed_kan_tile_ids": [],
                "added_kan_options": [],
                "can_nine_terminals": false
            },
            "result": null
        }))
        .expect("match view");
        app.screen = Screen::Game(GameScreen {
            view,
            selected_tile: 0,
            marked_tile_ids: Vec::new(),
        });
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("draw");

        let screen = screen_text(terminal.backend().buffer());
        assert!(screen.contains("1萬"));
        assert!(screen.contains("5筒"));
        assert!(screen.contains("東"));
        assert!(screen.contains("t自摸"));
        assert!(!screen.contains("9九种九牌"));
    }

    fn screen_text(buffer: &ratatui::buffer::Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .filter(|symbol| !symbol.trim().is_empty())
            .collect::<Vec<_>>()
            .join("")
    }
}
