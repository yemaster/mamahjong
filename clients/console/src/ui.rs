use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use unicode_width::UnicodeWidthChar;

use crate::app::{App, AuthMode, GameScreen, RoomBrowser, RoomScreen, Screen};
use crate::layout::{
    Density, MIN_HEIGHT, MIN_WIDTH, Spacing, centered, columns_with_gap, frame_bands, inset,
    page_area, ratio_columns, rows_with_gap, scroll_offset,
};
use crate::model::{
    MatchPlayerView, MatchResultView, MatchView, MatchmakingTicketView, ReactionOptionView,
    RoomView, TileView,
};
use crate::rules::{CreateRoomForm, RulePage, snapshot_summary};
use crate::stream::SeatCountdown;

const FELT: Color = Color::Rgb(18, 82, 64);
const IVORY: Color = Color::Rgb(245, 238, 214);
const GOLD: Color = Color::Rgb(230, 184, 82);
const INK: Color = Color::Rgb(10, 35, 31);
const MUTED: Color = Color::Rgb(153, 166, 160);
const RULE: Color = Color::Rgb(58, 70, 66);
const CREATE_SUMMARY_WIDTH: u16 = 30;
const CREATE_FIELDS_MIN_WIDTH: u16 = 64;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }
    let density = Density::of(area);
    let spacing = Spacing::for_density(density);
    let [header, body, footer] = frame_bands(area, spacing);
    let content = page_area(body, spacing);

    render_header(frame, header, app, spacing);
    match &app.screen {
        Screen::Auth(form) => render_auth(frame, content, form),
        Screen::Rooms(browser) => render_rooms(frame, content, browser, spacing, density),
        Screen::CreateRoom(form) => render_create(frame, content, form, spacing, density),
        Screen::Matchmaking(ticket) => render_matchmaking(frame, content, ticket),
        Screen::Room(screen) => render_room(frame, content, screen, app, spacing, density),
        Screen::Game(game) => render_game(frame, content, game, spacing, density),
    }
    render_footer(frame, footer, app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, spacing: Spacing) {
    frame.render_widget(Block::default().style(Style::default().bg(INK)), area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inset(area, spacing.page_horizontal, 0));
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(18)])
        .split(rows[0]);
    let screen = match &app.screen {
        Screen::Auth(_) => "登录",
        Screen::Rooms(_) => "大厅",
        Screen::CreateRoom(_) => "创建房间",
        Screen::Matchmaking(_) => "段位匹配",
        Screen::Room(screen) => screen.room.name.as_str(),
        Screen::Game(_) => "对局",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "麻麻的将",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  ", Style::default().fg(RULE)),
            Span::styled(screen, Style::default().fg(IVORY)),
        ]))
        .style(Style::default().bg(INK)),
        columns[0],
    );
    let user = app
        .user
        .as_ref()
        .map_or("", |user| user.profile.nickname.as_str());
    frame.render_widget(
        Paragraph::new(user)
            .alignment(Alignment::Right)
            .style(Style::default().fg(MUTED).bg(INK)),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(rows[1].width)))
            .style(Style::default().fg(RULE).bg(INK)),
        rows[1],
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let background = Color::Rgb(24, 30, 28);
    frame.render_widget(
        Block::default().style(Style::default().bg(background)),
        area,
    );
    let inner = inset(area, 1, 0);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(inner.width))).style(Style::default().fg(RULE)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(app.status.as_str()).style(Style::default().fg(IVORY)),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(help_text(app)).style(Style::default().fg(MUTED)),
        rows[2],
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
        Screen::CreateRoom(_) => "↑↓ 字段  [ ] 页签  ←→ 修改  Ctrl+S 创建  Esc 返回".to_owned(),
        Screen::Matchmaking(_) => "Esc 取消匹配".to_owned(),
        Screen::Room(screen) => {
            let ready = current_user_ready(&screen.room, app);
            format!(
                "Space {}  s 开始  ↑↓ 规则  r 刷新  Esc 离开",
                if ready { "取消准备" } else { "准备" }
            )
        }
        Screen::Game(game) if game.view.result.is_some() => "Esc 返回房间  q 退出".to_owned(),
        Screen::Game(game) => game_help(&game.view),
    }
}

fn game_help(view: &MatchView) -> String {
    if view.assets_loading() {
        return format!(
            "等待其他玩家({}/{})",
            view.assets_ready_seats.len(),
            view.players.len()
        );
    }
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

fn render_rooms(
    frame: &mut Frame<'_>,
    area: Rect,
    browser: &RoomBrowser,
    spacing: Spacing,
    density: Density,
) {
    let [list_area, quick_area] = if density.is_compact() {
        rows_with_gap(area, spacing, 7)
    } else {
        columns_with_gap(area, spacing, 30)
    };
    let columns = [list_area, quick_area];
    let items = if browser.rooms.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "暂无公开房间",
            Style::default().fg(MUTED),
        )))]
    } else {
        browser
            .rooms
            .iter()
            .enumerate()
            .map(|(index, room)| {
                let selected = index == browser.selected;
                let content = Style::default().fg(if selected { GOLD } else { Color::White });
                ListItem::new(Line::from(vec![
                    Span::styled(if selected { "▸ " } else { "  " }, content),
                    Span::styled(pad(&room.name, 20), content),
                    Span::styled(
                        pad(&format!("{}/{}", room.members.len(), seat_count(room)), 6),
                        content,
                    ),
                    Span::styled(pad(variant_label(room), 6), content),
                    Span::styled(lifecycle_label(&room.lifecycle), content),
                ]))
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

fn render_create(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &CreateRoomForm,
    spacing: Spacing,
    density: Density,
) {
    let show_summary = !density.is_compact()
        && area.width >= CREATE_FIELDS_MIN_WIDTH + spacing.panel_gap + CREATE_SUMMARY_WIDTH;
    let (fields_area, summary_area) = if show_summary {
        let [fields, summary] = columns_with_gap(area, spacing, CREATE_SUMMARY_WIDTH);
        (fields, Some(summary))
    } else {
        (area, None)
    };

    render_create_fields(frame, fields_area, form, spacing);
    if let Some(summary_area) = summary_area {
        render_create_summary(frame, summary_area, form, spacing);
    }
}

fn render_create_fields(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &CreateRoomForm,
    spacing: Spacing,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(tab_strip(form));
    let inner = inset(
        block.inner(area),
        spacing.panel_horizontal,
        spacing.panel_vertical,
    );
    frame.render_widget(block, area);

    let [list_area, hint_area] = {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        [rows[0], rows[1]]
    };

    let fields = form.page_fields();
    let unit = 1 + usize::from(spacing.field_gap);
    let visible = (usize::from(list_area.height) / unit).max(1);
    let offset = scroll_offset(form.active_field, fields.len(), visible);
    let label_width = 12;

    let mut lines = Vec::new();
    for (index, field) in fields.iter().enumerate().skip(offset).take(visible) {
        let active = index == form.active_field;
        let unavailable = form.field_unavailable(*field);
        let value_style = if unavailable {
            Style::default().fg(RULE)
        } else if active {
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(if active { "▸ " } else { "  " }, value_style),
            Span::styled(
                pad(CreateRoomForm::field_label(*field), label_width),
                Style::default().fg(if unavailable { RULE } else { MUTED }),
            ),
            Span::styled(form.field_value(*field), value_style),
        ]));
        for _ in 0..spacing.field_gap {
            lines.push(Line::from(""));
        }
    }
    frame.render_widget(Paragraph::new(lines), list_area);

    let (hint, style) = match form.validation_message() {
        Some(message) => (message, Style::default().fg(Color::LightRed)),
        None => (
            format!("字段 {}/{}", form.active_field + 1, fields.len()),
            Style::default().fg(MUTED),
        ),
    };
    frame.render_widget(Paragraph::new(hint).style(style), hint_area);
}

fn tab_strip(form: &CreateRoomForm) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for page in RulePage::ALL {
        let active = page == form.page;
        spans.push(Span::styled(
            page.title(),
            if active {
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            },
        ));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn render_create_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &CreateRoomForm,
    spacing: Spacing,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" 当前配置 ");
    let inner = inset(
        block.inner(area),
        spacing.panel_horizontal,
        spacing.panel_vertical,
    );
    frame.render_widget(block, area);
    let lines = form
        .summary()
        .into_iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(pad(label, 10), Style::default().fg(MUTED)),
                Span::styled(value, Style::default().fg(Color::White)),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_room(
    frame: &mut Frame<'_>,
    area: Rect,
    screen: &RoomScreen,
    _app: &App,
    spacing: Spacing,
    density: Density,
) {
    let room = &screen.room;
    let [members_area, rules_area] = if density.is_compact() {
        rows_with_gap(area, spacing, area.height.saturating_sub(8).min(10))
    } else {
        ratio_columns(area, spacing, 3, 2)
    };

    let members_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(
            " 成员 {}/{} ",
            room.members.len(),
            seat_count(room)
        ));
    let members_inner = inset(
        members_block.inner(members_area),
        spacing.panel_horizontal,
        spacing.panel_vertical,
    );
    frame.render_widget(members_block, members_area);
    let members = room
        .members
        .iter()
        .map(|member| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    pad(&format!("{}家", wind_for_seat(member.seat)), 6),
                    Style::default().fg(GOLD),
                ),
                Span::styled(pad(&member.nickname, 16), Style::default().fg(Color::White)),
                Span::styled(
                    pad(
                        if member.ready {
                            "已准备"
                        } else {
                            "等待中"
                        },
                        8,
                    ),
                    Style::default().fg(if member.ready { GOLD } else { MUTED }),
                ),
                Span::styled(
                    if member.user_id == room.owner_user_id {
                        "房主"
                    } else {
                        ""
                    },
                    Style::default().fg(MUTED),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(members), members_inner);

    let rules_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" 规则 ");
    let rules_inner = inset(
        rules_block.inner(rules_area),
        spacing.panel_horizontal,
        spacing.panel_vertical,
    );
    frame.render_widget(rules_block, rules_area);
    frame.render_widget(
        Paragraph::new(rule_summary_lines(&room.rule_snapshot)).scroll((screen.rules_scroll, 0)),
        rules_inner,
    );
}

fn rule_summary_lines(snapshot: &serde_json::Value) -> Vec<Line<'static>> {
    let groups = snapshot_summary(snapshot);
    let pick = |group: &str, label: &str| -> String {
        groups
            .iter()
            .find(|(name, _)| *name == group)
            .and_then(|(_, entries)| entries.iter().find(|(key, _)| *key == label))
            .map_or_else(|| "—".to_owned(), |(_, value)| value.clone())
    };
    let preset = snapshot["preset"]["id"]
        .as_str()
        .map_or_else(|| "普通规则".to_owned(), ToOwned::to_owned);
    let seats = if snapshot["rule_set_id"] == "riichi/sanma" {
        "三麻"
    } else {
        "四麻"
    };
    let mut lines = Vec::new();
    for (label, value) in [
        ("预设", preset),
        ("人数", seats.to_owned()),
        ("点数", pick("对局", "初始点数")),
        ("马点", pick("结算", "马点")),
        ("击飞", pick("对局", "击飞")),
        ("荣和", pick("结算", "荣和方式")),
    ] {
        lines.push(Line::from(vec![
            Span::styled(pad(label, 10), Style::default().fg(MUTED)),
            Span::styled(value, Style::default().fg(Color::White)),
        ]));
    }
    for (group, entries) in groups {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            group,
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )));
        for (label, value) in entries {
            lines.push(Line::from(vec![
                Span::styled(pad(label, 10), Style::default().fg(MUTED)),
                Span::styled(value, Style::default().fg(Color::White)),
            ]));
        }
    }
    lines
}

fn render_game(
    frame: &mut Frame<'_>,
    area: Rect,
    game: &GameScreen,
    spacing: Spacing,
    density: Density,
) {
    let view = &game.view;
    frame.render_widget(Block::default().style(Style::default().bg(FELT)), area);
    let hand_height = if density.is_compact() { 6 } else { 8 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Min(6),
            Constraint::Length(hand_height),
        ])
        .split(area);
    let relative = relative_seats(view);
    let countdown = |seat: u8| game.countdowns.iter().find(|c| c.seat == seat);
    let online = |seat: u8| game.online.get(usize::from(seat)).copied().unwrap_or(true);
    if let Some(top) = player(view, relative[2]) {
        render_player(
            frame,
            rows[0],
            top,
            false,
            0,
            &[],
            view.progress.dealer,
            countdown(top.seat),
            online(top.seat),
        );
    }
    let side = if density.is_compact() { 22 } else { 28 };
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(side),
            Constraint::Min(20),
            Constraint::Length(side),
        ])
        .split(rows[1]);
    if let Some(left) = player(view, relative[3]) {
        render_player(
            frame,
            middle[0],
            left,
            false,
            0,
            &[],
            view.progress.dealer,
            countdown(left.seat),
            online(left.seat),
        );
    }
    render_center(frame, inset(middle[1], spacing.panel_gap, 0), view);
    if let Some(right) = player(view, relative[1]) {
        render_player(
            frame,
            middle[2],
            right,
            false,
            0,
            &[],
            view.progress.dealer,
            countdown(right.seat),
            online(right.seat),
        );
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
            countdown(bottom.seat),
            online(bottom.seat),
        );
    }
    if let Some(result) = &view.result {
        render_result(frame, area, result);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_player(
    frame: &mut Frame<'_>,
    area: Rect,
    player: &MatchPlayerView,
    own: bool,
    selected: usize,
    marked: &[u16],
    dealer: u8,
    countdown: Option<&SeatCountdown>,
    online: bool,
) {
    let mut state = String::new();
    if player.seat == dealer {
        state.push_str(" · 亲");
    }
    if player.riichi_status == "established" {
        state.push_str(" · 立直");
    }
    if !online {
        state.push_str(" · 离线");
    }
    let title = if let Some(cd) = countdown {
        let total_s = cd.remaining_ms.div_ceil(1000);
        if cd.base_ms == 0 {
            format!(
                " {}家 · {} · {}点{} · 长考{total_s}s ",
                wind_for_seat(player.seat),
                player.nickname,
                player.points,
                state
            )
        } else {
            format!(
                " {}家 · {} · {}点{} · {total_s}s ",
                wind_for_seat(player.seat),
                player.nickname,
                player.points,
                state
            )
        }
    } else {
        format!(
            " {}家 · {} · {}点{} ",
            wind_for_seat(player.seat),
            player.nickname,
            player.points,
            state
        )
    };
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
            "供托 {} · 余牌 {}",
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

/// Pads or truncates to an exact column width so table columns stay aligned.
fn pad(text: &str, width: usize) -> String {
    let mut padded = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cells = character.width().unwrap_or(0);
        if used + cells > width {
            break;
        }
        padded.push(character);
        used += cells;
    }
    padded.push_str(&" ".repeat(width - used));
    padded
}

fn field_line(label: &str, value: &str, active: bool) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("  {}  ", pad(label, 6)), Style::default().fg(MUTED)),
        Span::styled(
            pad(value, 32),
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

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::{MIN_HEIGHT, MIN_WIDTH, render, tile_label};
    use crate::app::{App, GameScreen, RoomBrowser, RoomScreen, Screen};
    use crate::fixtures::{match_view, room_view, rule_catalog};
    use crate::rules::{CreateRoomForm, RulePage};

    /// The three documented breakpoints: compact, regular, wide.
    const SIZES: [(u16, u16); 3] = [(76, 22), (100, 30), (144, 42)];

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
        let screen = draw(&app, 90, 28);
        assert!(screen.contains("麻麻的将"));
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
    fn lobby_lists_rooms_at_every_breakpoint() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen = Screen::Rooms(RoomBrowser {
            rooms: vec![room_view("riichi/yonma", 4), room_view("riichi/sanma", 3)],
            selected: 0,
        });
        for (width, height) in SIZES {
            let screen = draw(&app, width, height);
            assert!(screen.contains("公开房间(2)"), "{width}x{height}");
            assert!(screen.contains("东南战练习房"), "{width}x{height}");
            assert!(screen.contains("四麻"), "{width}x{height}");
            assert!(screen.contains("三麻"), "{width}x{height}");
            assert!(screen.contains("创建房间"), "{width}x{height}");
        }
    }

    #[test]
    fn create_room_shows_every_tab_and_the_active_page_fields() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen =
            Screen::CreateRoom(Box::new(CreateRoomForm::new(rule_catalog()).expect("form")));
        for (width, height) in SIZES {
            let screen = draw(&app, width, height);
            for page in RulePage::ALL {
                assert!(screen.contains(page.title()), "{width}x{height} {page:?}");
            }
            assert!(screen.contains("房间名"), "{width}x{height}");
            assert!(screen.contains("字段1/"), "{width}x{height}");
        }
    }

    #[test]
    fn create_room_lists_the_call_rules_on_their_tabs() {
        for (page, label, value) in [
            (RulePage::Match, "食替", "禁止"),
            (RulePage::Scoring, "食断", "有"),
        ] {
            let mut form = CreateRoomForm::new(rule_catalog()).expect("form");
            form.page = page;
            let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
            app.screen = Screen::CreateRoom(Box::new(form));
            for (width, height) in SIZES {
                let screen = draw(&app, width, height);
                assert!(screen.contains(label), "{width}x{height} {label}");
                assert!(screen.contains(value), "{width}x{height} {label}");
            }
        }
    }

    #[test]
    fn create_room_reports_invalid_input_instead_of_the_field_counter() {
        let mut form = CreateRoomForm::new(rule_catalog()).expect("form");
        form.page = RulePage::Match;
        form.initial_points = "abc".to_owned();
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen = Screen::CreateRoom(Box::new(form));

        let screen = draw(&app, 100, 30);
        assert!(screen.contains("初始点数"));
        assert!(!screen.contains("字段1/"));
    }

    #[test]
    fn create_room_drops_the_summary_panel_when_the_terminal_is_narrow() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen =
            Screen::CreateRoom(Box::new(CreateRoomForm::new(rule_catalog()).expect("form")));
        assert!(!draw(&app, 76, 22).contains("当前配置"));
        assert!(draw(&app, 144, 42).contains("当前配置"));
    }

    #[test]
    fn room_groups_the_rule_snapshot_next_to_the_seat_list() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen = Screen::Room(RoomScreen::new(room_view("riichi/yonma", 4)));
        for (width, height) in SIZES {
            let screen = draw(&app, width, height);
            assert!(screen.contains("成员2/4"), "{width}x{height}");
            assert!(screen.contains("已准备"), "{width}x{height}");
            assert!(screen.contains("房主"), "{width}x{height}");
            assert!(screen.contains("预设普通规则"), "{width}x{height}");
        }
    }

    #[test]
    fn room_rule_panel_scrolls_to_the_later_groups() {
        let mut screen_state = RoomScreen::new(room_view("riichi/yonma", 4));
        screen_state.rules_scroll = 24;
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        app.screen = Screen::Room(screen_state);

        let screen = draw(&app, 144, 42);
        assert!(screen.contains("结算"));
        assert!(!screen.contains("预设普通规则"));
    }

    #[test]
    fn table_renders_all_seats_for_yonma_and_sanma() {
        for seats in [4_u8, 3] {
            let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
            let seat_count = usize::from(seats);
            app.screen = Screen::Game(GameScreen {
                view: match_view(seats),
                selected_tile: 0,
                marked_tile_ids: Vec::new(),
                countdowns: Vec::new(),
                online: vec![true; seat_count],
            });
            for (width, height) in SIZES {
                let screen = draw(&app, width, height);
                for seat in 0..seats {
                    assert!(
                        screen.contains(&format!("玩家{}", seat + 1)),
                        "{width}x{height} seats={seats} seat={seat}"
                    );
                }
                assert!(screen.contains("1萬"), "{width}x{height} seats={seats}");
                assert!(screen.contains("东1局"), "{width}x{height} seats={seats}");
            }
        }
    }

    #[test]
    fn game_screen_uses_localized_tiles_and_only_legal_actions() {
        let mut app = App::new("http://127.0.0.1:8080".to_owned()).expect("app");
        let mut view = match_view(4);
        view.turn_actions.can_tsumo = true;
        app.screen = Screen::Game(GameScreen {
            view,
            selected_tile: 0,
            marked_tile_ids: Vec::new(),
            countdowns: Vec::new(),
            online: vec![true; 4],
        });

        let screen = draw(&app, 120, 40);
        assert!(screen.contains("1萬"));
        assert!(screen.contains("5筒"));
        assert!(screen.contains("t自摸"));
        assert!(!screen.contains("9九种九牌"));
    }

    fn draw(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, app)).expect("draw");
        screen_text(terminal.backend().buffer())
    }

    fn screen_text(buffer: &Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .filter(|symbol| !symbol.trim().is_empty())
            .collect::<Vec<_>>()
            .join("")
    }
}
