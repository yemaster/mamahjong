use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use colored::Colorize;
use serde_json::json;

use crate::api::{ApiClient, ApiError};
use crate::model::{HandSettlementView, KanPointsView, MatchPhase, MatchResultView, MatchView};
use crate::strategy::{self, BotCommand};

/// Pause after every game command, so the table does not lurch forward.
const ACTION_PAUSE: Duration = Duration::from_secs(4);
/// Pause before deciding, so a move does not appear instantly.
const THINKING_PAUSE: Duration = Duration::from_secs(1);
/// Delay between polls while another seat is deciding.
const POLL_DELAY: Duration = Duration::from_secs(2);
/// Per-yaku animation step for settlement display.
const SETTLEMENT_YAKU_STEP: Duration = Duration::from_millis(300);
/// Base pause after the last yaku is revealed.
const SETTLEMENT_BASE_PAUSE: Duration = Duration::from_millis(300);
/// Extra pause for riichi-specific settlement details (ura dora, etc.).
const SETTLEMENT_RIICHI_EXTRA: Duration = Duration::from_millis(300);
/// Per-player kan-delta animation step.
const KAN_PLAYER_STEP: Duration = Duration::from_millis(300);
/// Base pause for kan animation.
const KAN_BASE_PAUSE: Duration = Duration::from_millis(500);
/// Per-tile dealing animation step.
const DEALING_TILE_STEP: Duration = Duration::from_millis(80);
/// Base pause for dealing animation.
const DEALING_BASE_PAUSE: Duration = Duration::from_millis(500);
/// Small pause for the asset-loading screen before reporting ready.
const LOADING_PAUSE: Duration = Duration::from_secs(1);
/// Delay between polls while the table is still dealing or still settling.
const HANDSHAKE_POLL_DELAY: Duration = Duration::from_millis(700);
/// Give up if a handshake never clears.
const HANDSHAKE_MAX_POLLS: u32 = 90;
/// Max retries when a transient error (network, 5xx) occurs.
const MAX_RETRIES: u32 = 5;
/// Backoff between retry attempts.
const RETRY_BACKOFF: Duration = Duration::from_secs(3);
/// Max consecutive transient errors before giving up.
const MAX_CONSECUTIVE_ERRORS: u32 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    Yonma,
    Sanma,
    /// 冲击麻将（四人固定）。
    Impact,
}

impl Variant {
    pub const fn rule_set_id(self) -> &'static str {
        match self {
            Self::Yonma => "riichi/yonma",
            Self::Sanma => "riichi/sanma",
            Self::Impact => "impact/standard",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Yonma => "四麻",
            Self::Sanma => "三麻",
            Self::Impact => "冲击",
        }
    }

    pub fn tile_kinds(self) -> impl Iterator<Item = usize> {
        (0..34).filter(move |kind| !matches!(self, Self::Sanma) || !(1..=7).contains(kind))
    }
}

pub fn detect_variant(view: &MatchView) -> Variant {
    if view.is_impact() {
        return Variant::Impact;
    }
    match view.players.len() {
        3 => Variant::Sanma,
        _ => Variant::Yonma,
    }
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub server_url: String,
    pub max_commands: u32,
    pub quiet: bool,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub variant: Variant,
    pub commands: u32,
    pub hands: u32,
    pub calls: u32,
    pub riichi: u32,
    pub wins: u32,
    pub result: MatchResultView,
}

impl RunReport {
    pub fn summary(&self) -> String {
        let placements = self
            .result
            .placements
            .iter()
            .map(|placement| {
                format!(
                    "{}位:{}家{}点",
                    placement.rank,
                    wind(placement.seat),
                    placement.points
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        if self.variant == Variant::Impact {
            format!(
                "{}完成 · {}局 · {}条命令 · 副露{} · 和牌{} · {}",
                self.variant.label(),
                self.hands,
                self.commands,
                self.calls,
                self.wins,
                placements
            )
        } else {
            format!(
                "{}完成 · {}局 · {}条命令 · 立直{} · 副露{} · 和牌{} · {} · {}",
                self.variant.label(),
                self.hands,
                self.commands,
                self.riichi,
                self.calls,
                self.wins,
                self.result.end_reason,
                placements
            )
        }
    }
}

#[derive(Default)]
pub struct RunStats {
    pub commands: u32,
    pub max_hand_index: u32,
    pub calls: u32,
    pub riichi: u32,
    pub wins: u32,
}

/// Display the initial game state.
pub fn display_initial_state(view: &MatchView) -> Result<(), String> {
    if view.is_impact() {
        return display_initial_state_impact(view);
    }
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "手牌不可见".to_owned())?;

    let b = "│".dimmed();
    let top = "┌────────────────────────────────────┐".dimmed();
    let mid = "├────────────────────────────────────┤".dimmed();
    let bot = "└────────────────────────────────────┘".dimmed();

    println!();
    println!("{}", top);
    println!(
        "{b}  {:^32}  {b}",
        "🀫  对局开始！  🀫".bright_yellow().bold()
    );
    println!("{b}  {:32}  {b}", "");
    println!("{}", mid);
    println!("{b}  {} {}", "对局:".dimmed(), view.id.cyan());
    println!(
        "{b}  {} {}{}局 · {}家 · {}点",
        "轮次:".dimmed(),
        wind_name(&view.progress.round_wind),
        view.progress.round_number,
        wind(player.seat),
        player.points
    );
    print!("{b}  {} ", "手牌:".dimmed());
    let mut sorted: Vec<_> = tiles.iter().collect();
    sorted.sort_by_key(|tile| tile_sort_key(&tile.code));
    for tile in &sorted {
        print!("{} ", color_tile(&tile.code, false));
    }
    println!();
    if !view.dora_indicators.is_empty() {
        print!("{b}  {} ", "宝牌:".dimmed());
        for tile in &view.dora_indicators {
            print!("{} ", color_tile(&tile.code, false));
        }
        println!();
    }
    if !player.melds.is_empty() {
        print!("{b}  {} ", "副露:".dimmed());
        for meld in &player.melds {
            for tile in &meld.tiles {
                print!("{} ", color_tile(&tile.code, false));
            }
            print!("| ");
        }
        println!();
    }
    println!("{}", bot);
    println!();
    println!("{}", "▶ 自动打牌开始".bright_green().bold());
    println!();
    Ok(())
}

fn display_initial_state_impact(view: &MatchView) -> Result<(), String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "手牌不可见".to_owned())?;

    let b = "│".dimmed();
    let top = "┌────────────────────────────────────┐".dimmed();
    let mid = "├────────────────────────────────────┤".dimmed();
    let bot = "└────────────────────────────────────┘".dimmed();

    println!();
    println!("{}", top);
    println!(
        "{b}  {:^32}  {b}",
        "🀫  冲击麻将对局开始！  🀫".bright_yellow().bold()
    );
    println!("{b}  {:32}  {b}", "");
    println!("{}", mid);
    println!("{b}  {} {}", "对局:".dimmed(), view.id.cyan());
    let streak = view.dealer_streak.unwrap_or(0);
    println!(
        "{b}  {} 庄家{}家 · 连庄{} · {}点",
        "轮次:".dimmed(),
        wind(view.progress.dealer),
        streak,
        player.points
    );
    if let Some(kp) = player.kan_points {
        println!("{b}  {} {}", "杠点:".dimmed(), kp);
    }
    print!("{b}  {} ", "手牌:".dimmed());
    let mut sorted: Vec<_> = tiles.iter().collect();
    sorted.sort_by_key(|tile| tile_sort_key(&tile.code));
    for tile in &sorted {
        let is_joker = view.joker_code().is_some_and(|code| tile.code == code);
        print!("{} ", color_tile(&tile.code, is_joker));
    }
    println!();
    if let Some(ref indicator) = view.joker_indicator {
        println!(
            "{b}  {} {}",
            "财神:".dimmed(),
            color_tile(&indicator.code, false)
        );
    }
    if let Some(ref code) = view.joker_code {
        println!(
            "{b}  {} {} → {}",
            "财神牌码:".dimmed(),
            code.dimmed(),
            "百搭".bright_magenta()
        );
    }
    if !player.melds.is_empty() {
        print!("{b}  {} ", "副露:".dimmed());
        for meld in &player.melds {
            for tile in &meld.tiles {
                print!("{} ", color_tile(&tile.code, false));
            }
            print!("| ");
        }
        println!();
    }
    println!("{}", bot);
    println!();
    println!("{}", "▶ 自动打牌开始".bright_green().bold());
    println!();
    Ok(())
}

/// Auto-play loop — dispatches to the riichi or impact path based on the view.
pub async fn auto_play(
    client: &ApiClient,
    match_id: &str,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchResultView, RunError> {
    let view = client.match_view(match_id).await?;
    let variant = detect_variant(&view);

    if variant == Variant::Impact {
        auto_play_impact(client, match_id, config, stats).await
    } else {
        auto_play_riichi(client, match_id, variant, config, stats).await
    }
}

/// Riichi (四麻 / 三麻) auto-play loop.
async fn auto_play_riichi(
    client: &ApiClient,
    match_id: &str,
    variant: Variant,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchResultView, RunError> {
    let mut view = poll_view(client, match_id).await?;
    let mut shown_settlement: Option<u32> = None;
    let mut acked_opening: Option<u32> = None;
    let mut acked_assets = false;
    let mut idle_polls: u32 = 0;
    let mut consecutive_errors: u32 = 0;
    loop {
        stats.max_hand_index = stats.max_hand_index.max(view.hand_index);

        if view.terminated_by_exit_vote {
            println!();
            println!("{} {}", "■".yellow(), "对局因退出投票提前结束".yellow());
            return Err(RunError::State("对局被退出投票终止".to_owned()));
        }

        if view.terminated_by_asset_timeout {
            println!();
            println!(
                "{} {}",
                "■".yellow(),
                "有玩家出现网络问题，对局已终止".yellow()
            );
            return Err(RunError::State("对局因玩家加载超时终止".to_owned()));
        }

        if view.needs_assets_ready() && !acked_assets {
            acked_assets = true;
            idle_polls = 0;
            tokio::time::sleep(LOADING_PAUSE).await;
            view =
                send_with_retry(client, match_id, view.version, "game.assets_ready", None).await?;
            continue;
        }

        if view.assets_loading() {
            view = wait(client, match_id, &mut idle_polls, "载入").await?;
            continue;
        }

        if stats.commands >= config.max_commands {
            return Err(RunError::State(format!(
                "超过最大命令数 {}，对局可能停滞",
                config.max_commands
            )));
        }

        if view.needs_exit_vote() {
            println!(
                "{} {}",
                "?".bright_yellow().bold(),
                "其他玩家发起了退出投票，机器人同意".yellow()
            );
            view = control(
                client,
                match_id,
                &view,
                "game.vote_exit",
                json!({"agree": true}),
            )
            .await?;
            continue;
        }

        if let Some(settlement) = view.hand_settlement.clone() {
            if shown_settlement != Some(view.hand_index) {
                display_settlement(&view, &settlement);
                shown_settlement = Some(view.hand_index);
            }
            if view.unplayed_settlement().is_some() {
                idle_polls = 0;
                tokio::time::sleep(settlement_duration(&settlement, false)).await;
                view = control(
                    client,
                    match_id,
                    &view,
                    "riichi.settlement_played",
                    json!({"hand_index": view.hand_index}),
                )
                .await?;
                continue;
            }
            if view.unconfirmed_settlement().is_some() {
                idle_polls = 0;
                view = control(
                    client,
                    match_id,
                    &view,
                    "riichi.confirm_settlement",
                    json!({"hand_index": view.hand_index}),
                )
                .await?;
                continue;
            }
            if view.result.is_none() {
                view = wait(client, match_id, &mut idle_polls, "结算").await?;
                continue;
            }
        }

        if let Some(result) = view.result.clone() {
            return Ok(result);
        }

        if view.needs_opening_ready() && acked_opening != Some(view.hand_index) {
            acked_opening = Some(view.hand_index);
            idle_polls = 0;
            let tile_count = view
                .observer()
                .map(|p| {
                    p.concealed_tiles
                        .as_deref()
                        .map(|tiles| tiles.len())
                        .unwrap_or(13)
                })
                .unwrap_or(13);
            tokio::time::sleep(dealing_duration(tile_count)).await;
            view = control(
                client,
                match_id,
                &view,
                "riichi.ready_for_hand",
                json!({"hand_index": view.hand_index}),
            )
            .await?;
            continue;
        }

        if view.opening_in_progress() {
            view = wait(client, match_id, &mut idle_polls, "发牌").await?;
            continue;
        }

        idle_polls = 0;
        let phase_result =
            dispatch_riichi_phase(client, match_id, &view, variant, config, stats).await;
        view = match phase_result {
            Ok(v) => {
                consecutive_errors = 0;
                v
            }
            Err(RunError::Strategy(message)) => {
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("策略错误: {message}").yellow()
                );
                consecutive_errors += 1;
                if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                    return Err(RunError::State(format!(
                        "连续{consecutive_errors}次策略错误，放弃重试"
                    )));
                }
                tokio::time::sleep(RETRY_BACKOFF).await;
                poll_view(client, match_id).await?
            }
            Err(error) if consecutive_errors < MAX_CONSECUTIVE_ERRORS => {
                consecutive_errors += 1;
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("操作失败 (第{consecutive_errors}次重试): {error}").yellow()
                );
                tokio::time::sleep(RETRY_BACKOFF).await;
                poll_view(client, match_id).await?
            }
            Err(error) => return Err(error),
        };
    }
}

/// Impact (冲击麻将) auto-play loop — resilient to transient errors.
async fn auto_play_impact(
    client: &ApiClient,
    match_id: &str,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchResultView, RunError> {
    let mut view = poll_view(client, match_id).await?;
    let mut shown_settlement: Option<u32> = None;
    let mut acked_opening: Option<u32> = None;
    let mut acked_assets = false;
    // Kan animation id already acknowledged — prevents hot-looping a kan that
    // the server keeps reflecting.
    let mut acked_kan_id: Option<u64> = None;
    let mut idle_polls: u32 = 0;
    let mut consecutive_errors: u32 = 0;
    loop {
        stats.max_hand_index = stats.max_hand_index.max(view.hand_index);

        if view.terminated_by_exit_vote {
            println!();
            println!("{} {}", "■".yellow(), "对局因退出投票提前结束".yellow());
            return Err(RunError::State("对局被退出投票终止".to_owned()));
        }

        if view.terminated_by_asset_timeout {
            println!();
            println!(
                "{} {}",
                "■".yellow(),
                "有玩家出现网络问题，对局已终止".yellow()
            );
            return Err(RunError::State("对局因玩家加载超时终止".to_owned()));
        }

        if view.needs_assets_ready() && !acked_assets {
            acked_assets = true;
            idle_polls = 0;
            tokio::time::sleep(LOADING_PAUSE).await;
            view =
                send_with_retry(client, match_id, view.version, "game.assets_ready", None).await?;
            continue;
        }

        if view.assets_loading() {
            view = wait(client, match_id, &mut idle_polls, "载入").await?;
            continue;
        }

        if stats.commands >= config.max_commands {
            return Err(RunError::State(format!(
                "超过最大命令数 {}，对局可能停滞",
                config.max_commands
            )));
        }

        if view.needs_exit_vote() {
            println!(
                "{} {}",
                "?".bright_yellow().bold(),
                "其他玩家发起了退出投票，机器人同意".yellow()
            );
            view = control(
                client,
                match_id,
                &view,
                "game.vote_exit",
                json!({"agree": true}),
            )
            .await?;
            continue;
        }

        // Kan animation handshake: every seat must ack before play resumes.
        if let Some(kan) = view.unplayed_kan() {
            if acked_kan_id != Some(kan.id) {
                acked_kan_id = Some(kan.id);
                idle_polls = 0;
                tokio::time::sleep(kan_animation_duration(kan)).await;
                view = control(
                    client,
                    match_id,
                    &view,
                    "impact.kan_animation_played",
                    json!({"kan_id": kan.id}),
                )
                .await?;
                continue;
            }
        }

        // Settlement — same two-step handshake as riichi.
        if let Some(settlement) = view.hand_settlement.clone() {
            if shown_settlement != Some(view.hand_index) {
                display_settlement_impact(&view, &settlement);
                shown_settlement = Some(view.hand_index);
            }
            if view.unplayed_settlement().is_some() {
                idle_polls = 0;
                tokio::time::sleep(settlement_duration(&settlement, true)).await;
                view = control(
                    client,
                    match_id,
                    &view,
                    "riichi.settlement_played",
                    json!({"hand_index": view.hand_index}),
                )
                .await?;
                continue;
            }
            if view.unconfirmed_settlement().is_some() {
                idle_polls = 0;
                view = control(
                    client,
                    match_id,
                    &view,
                    "riichi.confirm_settlement",
                    json!({"hand_index": view.hand_index}),
                )
                .await?;
                continue;
            }
            if view.result.is_none() {
                view = wait(client, match_id, &mut idle_polls, "结算").await?;
                continue;
            }
        }

        if let Some(result) = view.result.clone() {
            return Ok(result);
        }

        // Opening ready.
        if view.needs_opening_ready() && acked_opening != Some(view.hand_index) {
            acked_opening = Some(view.hand_index);
            idle_polls = 0;
            let tile_count = view
                .observer()
                .map(|p| {
                    p.concealed_tiles
                        .as_deref()
                        .map(|tiles| tiles.len())
                        .unwrap_or(13)
                })
                .unwrap_or(13);
            tokio::time::sleep(dealing_duration(tile_count)).await;
            view = control(
                client,
                match_id,
                &view,
                "riichi.ready_for_hand",
                json!({"hand_index": view.hand_index}),
            )
            .await?;
            continue;
        }

        if view.opening_in_progress() {
            view = wait(client, match_id, &mut idle_polls, "发牌").await?;
            continue;
        }

        idle_polls = 0;
        // Impact: "awaiting_kan_animation" is mapped to AwaitingTurnAction by
        // the server DTO, so the kan-animation check above MUST come first.
        let phase_result = dispatch_impact_phase(client, match_id, &view, config, stats).await;
        view = match phase_result {
            Ok(v) => {
                consecutive_errors = 0;
                v
            }
            Err(RunError::Strategy(message)) => {
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("策略错误: {message}").yellow()
                );
                // Fallback: try to pass or discard a non-joker tile.
                match impact_safe_fallback(client, match_id, &view, config, stats).await {
                    Ok(v) => {
                        consecutive_errors = 0;
                        v
                    }
                    Err(_) => {
                        // If even the fallback fails, re-poll and try again next iteration.
                        eprintln!(
                            "{} {}",
                            "⚠".yellow(),
                            "备用操作也失败，刷新状态重试...".yellow()
                        );
                        consecutive_errors += 1;
                        if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                            return Err(RunError::State(format!(
                                "连续{consecutive_errors}次错误，放弃重试"
                            )));
                        }
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        poll_view(client, match_id).await?
                    }
                }
            }
            Err(error) if consecutive_errors < MAX_CONSECUTIVE_ERRORS => {
                consecutive_errors += 1;
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("操作失败 (第{consecutive_errors}次重试): {error}").yellow()
                );
                tokio::time::sleep(RETRY_BACKOFF).await;
                poll_view(client, match_id).await?
            }
            Err(error) => return Err(error),
        };
    }
}

// ---------------------------------------------------------------------------
// Riichi turn execution
// ---------------------------------------------------------------------------

async fn execute_turn(
    client: &ApiClient,
    match_id: &str,
    view: MatchView,
    command: BotCommand,
    variant: Variant,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    let version = view.version;
    match submit(client, match_id, version, &command, config, stats).await {
        Ok(view) => Ok(view),
        Err(RunError::Api(error))
            if error.is_invalid_command()
                && matches!(command.name, "riichi.tsumo" | "riichi.riichi_discard") =>
        {
            let current = poll_view(client, match_id).await?;
            let fallback =
                strategy::fallback_discard(&current, variant).map_err(RunError::Strategy)?;
            submit(client, match_id, current.version, &fallback, config, stats).await
        }
        Err(RunError::Api(error)) if error.is_stale_version() => {
            let current = poll_view(client, match_id).await?;
            let command = strategy::turn_command(&current, variant).map_err(RunError::Strategy)?;
            submit(client, match_id, current.version, &command, config, stats).await
        }
        Err(error) => Err(error),
    }
}

/// Impact turn execution: on tsumo rejection, fall back to the best discard.
async fn execute_impact_turn(
    client: &ApiClient,
    match_id: &str,
    view: MatchView,
    command: BotCommand,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    let version = view.version;
    match submit(client, match_id, version, &command, config, stats).await {
        Ok(view) => Ok(view),
        Err(RunError::Api(error))
            if error.is_invalid_command() && command.name == "impact.tsumo" =>
        {
            let current = poll_view(client, match_id).await?;
            let fallback =
                strategy::impact_fallback_discard(&current).map_err(RunError::Strategy)?;
            submit(client, match_id, current.version, &fallback, config, stats).await
        }
        Err(RunError::Api(error)) if error.is_stale_version() => {
            let current = poll_view(client, match_id).await?;
            let command = strategy::impact_turn_command(&current).map_err(RunError::Strategy)?;
            submit(client, match_id, current.version, &command, config, stats).await
        }
        Err(error) => Err(error),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Poll the match view with automatic retry on any error.
async fn poll_view(client: &ApiClient, match_id: &str) -> Result<MatchView, RunError> {
    let mut attempt = 0_u32;
    loop {
        match client.match_view(match_id).await {
            Ok(view) => return Ok(view),
            Err(error) if attempt < MAX_RETRIES => {
                attempt += 1;
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("轮询失败 (第{attempt}次重试): {error}").yellow()
                );
                tokio::time::sleep(RETRY_BACKOFF).await;
            }
            Err(error) => return Err(RunError::Api(error)),
        }
    }
}

async fn wait(
    client: &ApiClient,
    match_id: &str,
    idle_polls: &mut u32,
    what: &str,
) -> Result<MatchView, RunError> {
    *idle_polls += 1;
    if *idle_polls > HANDSHAKE_MAX_POLLS {
        return Err(RunError::State(format!("等待{what}超时，对局可能停滞")));
    }
    tokio::time::sleep(HANDSHAKE_POLL_DELAY).await;
    poll_view(client, match_id).await
}

async fn control(
    client: &ApiClient,
    match_id: &str,
    view: &MatchView,
    name: &'static str,
    payload: serde_json::Value,
) -> Result<MatchView, RunError> {
    let mut attempt = 0_u32;
    loop {
        match client
            .game_command(match_id, view.version, name, Some(payload.clone()))
            .await
        {
            Ok(view) => return Ok(view),
            Err(error) if error.is_stale_version() => {
                let current = poll_view(client, match_id).await?;
                return Ok(client
                    .game_command(match_id, current.version, name, Some(payload))
                    .await?);
            }
            Err(error) if error.is_invalid_command() => {
                tokio::time::sleep(POLL_DELAY).await;
                return poll_view(client, match_id).await;
            }
            Err(error) if attempt < MAX_RETRIES => {
                attempt += 1;
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("控制命令失败 (第{attempt}次重试): {error}").yellow()
                );
                tokio::time::sleep(RETRY_BACKOFF).await;
            }
            Err(error) => return Err(RunError::Api(error)),
        }
    }
}

/// Simple game command with retry on any error.
async fn send_with_retry(
    client: &ApiClient,
    match_id: &str,
    version: u64,
    name: &str,
    payload: Option<serde_json::Value>,
) -> Result<MatchView, RunError> {
    let mut attempt = 0_u32;
    loop {
        match client
            .game_command(match_id, version, name, payload.clone())
            .await
        {
            Ok(view) => return Ok(view),
            Err(error) if attempt < MAX_RETRIES => {
                attempt += 1;
                eprintln!(
                    "{} {}",
                    "⚠".yellow(),
                    format!("命令 {name} 失败 (第{attempt}次重试): {error}").yellow()
                );
                tokio::time::sleep(RETRY_BACKOFF).await;
            }
            Err(error) => return Err(RunError::Api(error)),
        }
    }
}

/// Dispatch the riichi-game phase.  Extracted so the outer loop can catch and
/// retry on transient errors.
async fn dispatch_riichi_phase(
    client: &ApiClient,
    match_id: &str,
    view: &MatchView,
    variant: Variant,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    match view.phase {
        MatchPhase::AwaitingTurnAction { seat } | MatchPhase::AwaitingDiscard { seat } => {
            if seat != view.observer_seat {
                tokio::time::sleep(POLL_DELAY).await;
                poll_view(client, match_id).await
            } else {
                tokio::time::sleep(THINKING_PAUSE).await;
                let command = strategy::turn_command(view, variant).map_err(RunError::Strategy)?;
                execute_turn(
                    client,
                    match_id,
                    view.clone(),
                    command,
                    variant,
                    config,
                    stats,
                )
                .await
            }
        }
        MatchPhase::AwaitingResponses { .. } => {
            if view.available_reactions.is_empty() {
                tokio::time::sleep(POLL_DELAY).await;
                poll_view(client, match_id).await
            } else {
                tokio::time::sleep(THINKING_PAUSE).await;
                let command = strategy::reaction_command(view, variant)
                    .map_err(RunError::Strategy)?
                    .ok_or_else(|| RunError::State("响应窗口没有可执行动作".to_owned()))?;
                match submit(client, match_id, view.version, &command, config, stats).await {
                    Ok(view) => Ok(view),
                    Err(RunError::Api(error)) if error.is_stale_version() => {
                        let current = poll_view(client, match_id).await?;
                        let command = strategy::reaction_command(&current, variant)
                            .map_err(RunError::Strategy)?
                            .ok_or_else(|| RunError::State("响应窗口没有可执行动作".to_owned()))?;
                        submit(client, match_id, current.version, &command, config, stats).await
                    }
                    Err(error) => Err(error),
                }
            }
        }
        MatchPhase::AwaitingKanAnimation { .. } => {
            // Kan animation is handled by the outer loop via `unplayed_kan()`.
            // This phase should never be reached because the outer loop processes
            // the kan before dispatching to this function.
            tokio::time::sleep(POLL_DELAY).await;
            poll_view(client, match_id).await
        }
        MatchPhase::Ended { .. } => {
            tokio::time::sleep(POLL_DELAY).await;
            poll_view(client, match_id).await
        }
    }
}

/// Dispatch the impact-game phase: turn action, reaction, or ended.
/// Extracted so the outer loop can catch and retry on transient errors.
async fn dispatch_impact_phase(
    client: &ApiClient,
    match_id: &str,
    view: &MatchView,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    match view.phase {
        MatchPhase::AwaitingTurnAction { seat } | MatchPhase::AwaitingDiscard { seat } => {
            if seat != view.observer_seat {
                tokio::time::sleep(POLL_DELAY).await;
                poll_view(client, match_id).await
            } else {
                tokio::time::sleep(THINKING_PAUSE).await;
                let command = strategy::impact_turn_command(view).map_err(RunError::Strategy)?;
                execute_impact_turn(client, match_id, view.clone(), command, config, stats).await
            }
        }
        MatchPhase::AwaitingResponses { .. } => {
            if view.available_reactions.is_empty() {
                tokio::time::sleep(POLL_DELAY).await;
                poll_view(client, match_id).await
            } else {
                tokio::time::sleep(THINKING_PAUSE).await;
                let command = strategy::impact_reaction_command(view)
                    .map_err(RunError::Strategy)?
                    .ok_or_else(|| RunError::State("响应窗口没有可执行动作".to_owned()))?;
                match submit(client, match_id, view.version, &command, config, stats).await {
                    Ok(view) => Ok(view),
                    Err(RunError::Api(error)) if error.is_stale_version() => {
                        let current = poll_view(client, match_id).await?;
                        let command = strategy::impact_reaction_command(&current)
                            .map_err(RunError::Strategy)?
                            .ok_or_else(|| RunError::State("响应窗口没有可执行动作".to_owned()))?;
                        submit(client, match_id, current.version, &command, config, stats).await
                    }
                    Err(error) => Err(error),
                }
            }
        }
        MatchPhase::AwaitingKanAnimation { .. } => {
            // Kan animation is handled by the outer loop via `unplayed_kan()`.
            // This phase should never be reached because the outer loop processes
            // the kan before dispatching to this function.
            tokio::time::sleep(POLL_DELAY).await;
            poll_view(client, match_id).await
        }
        MatchPhase::Ended { .. } => {
            tokio::time::sleep(POLL_DELAY).await;
            poll_view(client, match_id).await
        }
    }
}

/// Safe fallback when the strategy module fails: try to pass, otherwise discard
/// the first non-joker tile.
async fn impact_safe_fallback(
    client: &ApiClient,
    match_id: &str,
    view: &MatchView,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    match view.phase {
        MatchPhase::AwaitingTurnAction { .. } | MatchPhase::AwaitingDiscard { .. } => {
            // Discard the first non-joker tile as a safety net.
            let player = view.observer().map_err(RunError::Strategy)?;
            let tiles = player
                .concealed_tiles
                .as_deref()
                .ok_or_else(|| RunError::Strategy("手牌不可见".to_owned()))?;
            let tile_id = tiles
                .iter()
                .find(|t| view.joker_code().is_none_or(|c| t.code != c))
                .or_else(|| tiles.first())
                .map(|t| t.id)
                .ok_or_else(|| RunError::Strategy("没有可弃的牌".to_owned()))?;
            let command = BotCommand {
                name: "impact.discard",
                description: format!("弃 {}（备用）", tile_id),
                payload: Some(json!({"tile_id": tile_id})),
            };
            submit(client, match_id, view.version, &command, config, stats).await
        }
        MatchPhase::AwaitingResponses { .. } => {
            // It's safer to pass than to make a bad call.
            send_with_retry(client, match_id, view.version, "impact.pass", None).await
        }
        _ => poll_view(client, match_id).await,
    }
}

async fn submit(
    client: &ApiClient,
    match_id: &str,
    expected_version: u64,
    command: &BotCommand,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    let view = client
        .game_command(
            match_id,
            expected_version,
            command.name,
            command.payload.clone(),
        )
        .await?;
    stats.commands += 1;
    if command.name == "riichi.riichi_discard" {
        stats.riichi += 1;
    }
    if matches!(
        command.name,
        "riichi.chi" | "riichi.pon" | "riichi.open_kan" | "impact.pon" | "impact.open_kan"
    ) {
        stats.calls += 1;
    }
    if matches!(command.name, "riichi.tsumo" | "riichi.ron" | "impact.tsumo") {
        stats.wins += 1;
    }
    if !config.quiet {
        let nickname = view.observer().map(|p| p.nickname.as_str()).unwrap_or("?");
        let round = round_label(&view);
        println!(
            "{} {} {} {} {}",
            round.bright_black(),
            format!("#{}", stats.commands).dimmed(),
            nickname.cyan(),
            "→".dimmed(),
            command.description.white()
        );
    }
    tokio::time::sleep(ACTION_PAUSE).await;
    Ok(view)
}

fn round_label(view: &MatchView) -> String {
    if view.is_impact() {
        format!(
            "庄{}·连{}",
            wind(view.progress.dealer),
            view.dealer_streak.unwrap_or(0)
        )
    } else {
        format!(
            "{}{}局",
            wind_name(&view.progress.round_wind),
            view.progress.round_number
        )
    }
}

// ---------------------------------------------------------------------------
// Animation-aware delays
// ---------------------------------------------------------------------------

/// Calculate how long to pause for the settlement animation.
///
/// Each yaku takes a ~300ms reveal step, followed by a 300ms hold at the end.
/// Riichi tables get an extra 300ms for ura-dora / limit indicators.
fn settlement_duration(settlement: &HandSettlementView, is_impact: bool) -> Duration {
    let yaku_count: u32 = settlement
        .winners
        .iter()
        .map(|winner| u32::try_from(winner.yaku.len()).unwrap_or(0))
        .sum();
    let mut duration = SETTLEMENT_YAKU_STEP * yaku_count + SETTLEMENT_BASE_PAUSE;
    if !is_impact {
        duration += SETTLEMENT_RIICHI_EXTRA;
    }
    duration
}

/// Calculate how long to pause for the kan-points animation.
///
/// Each player with a non-zero kan-point delta contributes one step.
fn kan_animation_duration(kan: &KanPointsView) -> Duration {
    let affected = kan.deltas.iter().filter(|d| **d != 0).count() as u32;
    KAN_PLAYER_STEP * affected.max(1) + KAN_BASE_PAUSE
}

/// Calculate how long to pause for the dealing animation.
///
/// Each tile dealt adds a small step.
fn dealing_duration(tile_count: usize) -> Duration {
    DEALING_TILE_STEP * (tile_count as u32).max(1) + DEALING_BASE_PAUSE
}

// ---------------------------------------------------------------------------
// Settlement display
// ---------------------------------------------------------------------------

fn display_settlement(view: &MatchView, settlement: &HandSettlementView) {
    let bar = "─".repeat(38);
    println!();
    println!("{}", bar.dimmed());
    println!(
        "  {} {}",
        "本局结束".bright_yellow().bold(),
        end_reason_name(&settlement.reason).yellow()
    );
    if settlement.winners.is_empty() {
        let tenpai = settlement
            .tenpai_seats
            .iter()
            .map(|seat| format!("{}家", wind(*seat)))
            .collect::<Vec<_>>()
            .join(" ");
        if tenpai.is_empty() {
            println!("  {}", "无人听牌".dimmed());
        } else {
            println!("  {} {}", "听牌:".dimmed(), tenpai);
        }
    }
    for winner in &settlement.winners {
        let score = if winner.yakuman_multiplier > 0 {
            winner.limit.clone()
        } else if winner.limit.is_empty() {
            format!("{}番{}符", winner.han, winner.fu)
        } else {
            format!("{}番{}符 {}", winner.han, winner.fu, winner.limit)
        };
        println!(
            "  {} {} {} {}",
            format!("{}家", wind(winner.seat)).bright_green().bold(),
            score.white(),
            format!("{}点", winner.points).bright_yellow(),
            if winner.dealer {
                "(庄)".dimmed()
            } else {
                "".dimmed()
            }
        );
        let yaku = winner
            .yaku
            .iter()
            .map(|yaku| {
                if yaku.yakuman {
                    yaku.name.clone()
                } else {
                    format!("{}({})", yaku.name, yaku.value)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        if !yaku.is_empty() {
            println!("    {}", yaku.dimmed());
        }
    }
    if !settlement.ura_dora_indicators.is_empty() {
        print!("  {} ", "里宝牌:".dimmed());
        for tile in &settlement.ura_dora_indicators {
            print!("{} ", color_tile(&tile.code, false));
        }
        println!();
    }
    print!("  {} ", "点数:".dimmed());
    for player in &view.players {
        let index = usize::from(player.seat);
        let delta = settlement.point_deltas.get(index).copied().unwrap_or(0);
        let after = settlement
            .points_after
            .get(index)
            .copied()
            .unwrap_or(player.points);
        let delta_text = match delta.cmp(&0) {
            std::cmp::Ordering::Greater => format!("+{delta}").bright_green().to_string(),
            std::cmp::Ordering::Less => delta.to_string().bright_red().to_string(),
            std::cmp::Ordering::Equal => "±0".dimmed().to_string(),
        };
        let marker = if player.seat == view.observer_seat {
            "*"
        } else {
            ""
        };
        print!("{}{}家 {after} {delta_text}   ", marker, wind(player.seat));
    }
    println!();
    println!("{}", bar.dimmed());
}

fn display_settlement_impact(view: &MatchView, settlement: &HandSettlementView) {
    let bar = "─".repeat(38);
    println!();
    println!("{}", bar.dimmed());
    if settlement.void_hand.unwrap_or(false) {
        println!(
            "  {} {}",
            "本局结束".bright_yellow().bold(),
            "荒牌流局（不算，重开）".yellow()
        );
    } else if settlement.all_in.is_some() {
        println!(
            "  {} {}",
            "本局结束".bright_yellow().bold(),
            "全交！".bright_magenta().bold()
        );
    } else {
        println!(
            "  {} {}",
            "本局结束".bright_yellow().bold(),
            end_reason_name(&settlement.reason).yellow()
        );
    }

    for winner in &settlement.winners {
        if settlement.all_in.is_some() {
            println!(
                "  {} {} {}",
                format!("{}家", wind(winner.seat)).bright_green().bold(),
                "全交".bright_magenta().bold(),
                format!("{}点", winner.points).bright_yellow(),
            );
        } else {
            println!(
                "  {} {} {}",
                format!("{}家", wind(winner.seat)).bright_green().bold(),
                format!("{}点", winner.points).bright_yellow(),
                if winner.dealer {
                    "(庄)".dimmed()
                } else {
                    "".dimmed()
                }
            );
        }
        let yaku = winner
            .yaku
            .iter()
            .map(|yaku| format!("{}({})", yaku.name, yaku.value))
            .collect::<Vec<_>>()
            .join(" ");
        if !yaku.is_empty() {
            println!("    {}", yaku.dimmed());
        }
    }

    if !settlement.winners.is_empty() {
        // Show kan-point deltas if present.
        if let Some(ref kp_deltas) = settlement.kan_point_deltas {
            let has_movement = kp_deltas.iter().any(|d| *d != 0);
            if has_movement {
                print!("  {} ", "杠点:".dimmed());
                for player in &view.players {
                    let index = usize::from(player.seat);
                    let delta = kp_deltas.get(index).copied().unwrap_or(0);
                    let after = settlement
                        .kan_points_after
                        .as_ref()
                        .and_then(|kp| kp.get(index).copied())
                        .unwrap_or(player.kan_points.unwrap_or(0));
                    if delta != 0 {
                        let delta_text = if delta > 0 {
                            format!("+{delta}").bright_green().to_string()
                        } else {
                            delta.to_string().bright_red().to_string()
                        };
                        print!(
                            "{}{}家 {after} {delta_text}   ",
                            if player.seat == view.observer_seat {
                                "*"
                            } else {
                                ""
                            },
                            wind(player.seat)
                        );
                    }
                }
                println!();
            }
        }
    }

    print!("  {} ", "点数:".dimmed());
    for player in &view.players {
        let index = usize::from(player.seat);
        let delta = settlement.point_deltas.get(index).copied().unwrap_or(0);
        let after = settlement
            .points_after
            .get(index)
            .copied()
            .unwrap_or(player.points);
        let delta_text = match delta.cmp(&0) {
            std::cmp::Ordering::Greater => format!("+{delta}").bright_green().to_string(),
            std::cmp::Ordering::Less => delta.to_string().bright_red().to_string(),
            std::cmp::Ordering::Equal => "±0".dimmed().to_string(),
        };
        let marker = if player.seat == view.observer_seat {
            "*"
        } else {
            ""
        };
        print!("{}{}家 {after} {delta_text}   ", marker, wind(player.seat));
    }
    println!();
    println!("{}", bar.dimmed());
}

fn end_reason_name(value: &str) -> &str {
    match value {
        "tsumo" => "自摸",
        "ron" => "荣和",
        "exhaustive_draw" => "荒牌流局",
        "nine_terminals" => "九种九牌",
        "four_winds" => "四风连打",
        "four_kans" => "四杠散了",
        "four_riichi" => "四家立直",
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn wind_name(value: &str) -> &str {
    match value {
        "east" => "东",
        "south" => "南",
        "west" => "西",
        "north" => "北",
        _ => "?",
    }
}

fn wind(seat: u8) -> &'static str {
    match seat {
        0 => "东",
        1 => "南",
        2 => "西",
        3 => "北",
        _ => "?",
    }
}

fn color_tile(code: &str, is_joker: bool) -> String {
    if is_joker {
        return code.bright_magenta().bold().to_string();
    }
    let bytes = code.as_bytes();
    if bytes.len() != 2 {
        return code.to_owned();
    }
    let suit = bytes[1];
    if bytes[0] == b'0' {
        return code.red().bold().to_string();
    }
    match suit {
        b'm' => code.bright_red().to_string(),
        b'p' => code.bright_blue().to_string(),
        b's' => code.bright_green().to_string(),
        b'z' => code.bright_yellow().to_string(),
        _ => code.to_owned(),
    }
}

fn tile_sort_key(code: &str) -> (usize, u8) {
    let bytes = code.as_bytes();
    if bytes.len() != 2 {
        return (99, 0);
    }
    let suit = match bytes[1] {
        b'm' => 0,
        b'p' => 1,
        b's' => 2,
        b'z' => 3,
        _ => 4,
    };
    let rank = if bytes[0] == b'0' {
        5
    } else {
        bytes[0].saturating_sub(b'0')
    };
    (suit, rank)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum RunError {
    Api(ApiError),
    State(String),
    Strategy(String),
}

impl Display for RunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api(error) => Display::fmt(error, formatter),
            Self::State(message) | Self::Strategy(message) => formatter.write_str(message),
        }
    }
}

impl Error for RunError {}

impl From<ApiError> for RunError {
    fn from(value: ApiError) -> Self {
        Self::Api(value)
    }
}
