use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use crate::app::{App, AuthMode, CreateRoomForm, GameScreen, RoomBrowser, Screen};
use crate::model::{MatchPlayerView, MatchResultView, MatchView, RoomView, TileView};

const FELT: Color = Color::Rgb(18, 82, 64);
const IVORY: Color = Color::Rgb(245, 238, 214);
const GOLD: Color = Color::Rgb(230, 184, 82);

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(" MAMAHJONG · 在线日麻 ")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(GOLD)
                    .bg(Color::Rgb(10, 35, 31))
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );
    match &app.screen {
        Screen::Auth(form) => render_auth(frame, chunks[1], form),
        Screen::Rooms(browser) => render_rooms(frame, chunks[1], browser),
        Screen::CreateRoom(form) => render_create(frame, chunks[1], form),
        Screen::Room(room) => render_room(frame, chunks[1], room, app),
        Screen::Game(game) => render_game(frame, chunks[1], game),
    }
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .style(Style::default().fg(Color::White).bg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("状态")),
        chunks[2],
    );
}

fn render_auth(frame: &mut Frame<'_>, area: Rect, form: &crate::app::AuthForm) {
    let width = area.width.min(62);
    let height = if form.mode == AuthMode::Register {
        15
    } else {
        12
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
    lines.push(Line::from(""));
    lines.push(Line::from("Enter 提交  ·  Tab 下一项  ·  F2 登录/注册"));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(title)),
        dialog,
    );
}

fn render_rooms(frame: &mut Frame<'_>, area: Rect, browser: &RoomBrowser) {
    let items = if browser.rooms.is_empty() {
        vec![ListItem::new("暂无公开房间，按 n 创建")]
    } else {
        browser
            .rooms
            .iter()
            .enumerate()
            .map(|(index, room)| {
                let marker = if index == browser.selected {
                    "▶"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{marker} {}  [{}/{}]  {} · {}",
                    room.name,
                    room.members.len(),
                    seat_count(room),
                    variant_label(room),
                    room.lifecycle
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
                .title("房间大厅 · ↑↓ 选择 / Enter 加入 / n 新建 / r 刷新 / q 退出"),
        ),
        area,
    );
}

fn render_create(frame: &mut Frame<'_>, area: Rect, form: &CreateRoomForm) {
    let dialog = centered(area, area.width.min(72), 17);
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
        Line::from(format!(
            "F3 人数：[{}]    F4 荣和：[{}]    F5 击飞：[{}]",
            if form.variant == "yonma" {
                "四麻"
            } else {
                "三麻"
            },
            if form.head_bump {
                "头跳"
            } else {
                "多家和"
            },
            if form.tobi { "有" } else { "无" }
        )),
        Line::from(""),
        Line::from("Enter 创建 · Tab 切换输入 · Esc 返回"),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("创建房间")),
        dialog,
    );
}

fn render_room(frame: &mut Frame<'_>, area: Rect, room: &RoomView, app: &App) {
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
                "{}位  {}  {}{}",
                member.seat + 1,
                member.nickname,
                if member.ready {
                    "✓ 已准备"
                } else {
                    "○ 等待"
                },
                owner
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(members).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} · 成员", room.name)),
        ),
        columns[0],
    );
    let config = &room.rule_snapshot["config"];
    let me = app.user.as_ref().map(|user| user.id.as_str());
    let ready = room
        .members
        .iter()
        .find(|member| Some(member.user_id.as_str()) == me)
        .is_some_and(|member| member.ready);
    let lines = vec![
        Line::from(format!("玩法：{}", variant_label(room))),
        Line::from(format!(
            "初始点数：{}",
            config["match_rules"]["initial_points"]
        )),
        Line::from(format!(
            "流局罚点：{}",
            config["settlement"]["noten_payment"]
        )),
        Line::from(format!(
            "荣和：{}",
            if config["settlement"]["ron_resolution"] == "head_bump" {
                "头跳"
            } else {
                "多家和"
            }
        )),
        Line::from(""),
        Line::from(if ready {
            "Space 取消准备"
        } else {
            "Space 准备"
        }),
        Line::from("s 开始（房主） · l 离开 · r 刷新"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("规则")),
        columns[1],
    );
}

fn render_game(frame: &mut Frame<'_>, area: Rect, game: &GameScreen) {
    let view = &game.view;
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
        render_player(frame, rows[0], top, false, 0, view.progress.dealer);
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
        render_player(frame, middle[0], left, false, 0, view.progress.dealer);
    }
    render_center(frame, middle[1], view);
    if let Some(right) = player(view, relative[1]) {
        render_player(frame, middle[2], right, false, 0, view.progress.dealer);
    }
    if let Some(bottom) = player(view, relative[0]) {
        render_player(
            frame,
            rows[2],
            bottom,
            true,
            game.selected_tile,
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
    dealer: u8,
) {
    let title = format!(
        "{}家 · {} · {}点{}",
        wind_for_seat(player.seat),
        player.nickname,
        player.points,
        if player.riichi_status == "established" {
            " · 立直"
        } else if player.seat == dealer {
            " · 亲"
        } else {
            ""
        }
    );
    let mut lines = Vec::new();
    let discards = player
        .discards
        .iter()
        .map(|discard| {
            if discard.riichi_declared {
                format!("{}*", discard.tile.code)
            } else if discard.claimed_by.is_some() {
                format!("({})", discard.tile.code)
            } else if discard.tsumogiri {
                format!("{}˙", discard.tile.code)
            } else {
                discard.tile.code.clone()
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
                    meld.kind,
                    meld.tiles
                        .iter()
                        .map(|tile| tile.code.as_str())
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
            .map(|tiles| tile_line(tiles, selected, player.drawn_tile_id))
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
        .map(|tile| tile.code.as_str())
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
            format!("等待响应 · {}家打牌", wind_for_seat(trigger_seat))
        }
        crate::model::MatchPhase::Ended { reason } => format!("本局结束 · {reason:?}"),
    };
    let lines = vec![
        Line::from(format!(
            "{}{}局  {}本场",
            wind_label(&view.progress.round_wind),
            view.progress.round_number,
            view.progress.honba
        )),
        Line::from(format!(
            "供托 {} · 余 {} 张 · 事件 #{}",
            view.progress.riichi_sticks, view.remaining_live_draws, view.event_sequence
        )),
        Line::from(format!("宝牌指示：{dora}")),
        Line::from(""),
        Line::from(phase),
        Line::from(""),
        Line::from("←→ 选牌 · d 打牌 · r 立直 · t 自摸"),
        Line::from("响应：p 过 · h 荣和 · 9 九种九牌 · q 退出"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default().fg(IVORY).bg(Color::Rgb(8, 55, 43)))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("桌心")),
        area,
    );
}

fn render_result(frame: &mut Frame<'_>, area: Rect, result: &MatchResultView) {
    let dialog = centered(area, area.width.min(56), 10);
    frame.render_widget(Clear, dialog);
    let mut lines = vec![Line::from(format!("结束原因：{}", result.end_reason))];
    for placement in &result.placements {
        lines.push(Line::from(format!(
            "{}位  {}家  {}点  成绩 {:+.1}",
            placement.rank,
            wind_for_seat(placement.seat),
            placement.points,
            f64::from(placement.score_tenths) / 10.0
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("q 退出客户端"));
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(GOLD))
                .title("整场结果"),
        ),
        dialog,
    );
}

fn tile_line(tiles: &[TileView], selected: usize, drawn: Option<u16>) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, tile) in tiles.iter().enumerate() {
        let mut style = Style::default().fg(Color::Black).bg(IVORY);
        if tile.code.starts_with('0') {
            style = style.fg(Color::Red);
        }
        if index == selected {
            style = style.bg(GOLD).add_modifier(Modifier::BOLD);
        }
        let marker = if Some(tile.id) == drawn { "˙" } else { " " };
        spans.push(Span::styled(format!(" {}{marker} ", tile.code), style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn field_line<'a>(label: &'a str, value: &'a str, active: bool) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!("{label:>8}："), Style::default().fg(Color::Gray)),
        Span::styled(
            if value.is_empty() { " " } else { value },
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
