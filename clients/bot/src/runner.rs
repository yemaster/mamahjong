use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::{ApiClient, ApiError};
use crate::model::{MatchPhase, MatchResultView, MatchView};
use crate::strategy::{self, BotCommand};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    Yonma,
    Sanma,
}

impl Variant {
    pub const fn seat_count(self) -> u8 {
        match self {
            Self::Yonma => 4,
            Self::Sanma => 3,
        }
    }

    pub const fn rule_set_id(self) -> &'static str {
        match self {
            Self::Yonma => "riichi/yonma",
            Self::Sanma => "riichi/sanma",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Yonma => "四麻",
            Self::Sanma => "三麻",
        }
    }

    pub fn tile_kinds(self) -> impl Iterator<Item = usize> {
        (0..34).filter(move |kind| !matches!(self, Self::Sanma) || !(1..=7).contains(kind))
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
    pub match_id: String,
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

struct Bot {
    client: ApiClient,
    user_id: String,
    seat: u8,
}

#[derive(Default)]
struct RunStats {
    commands: u32,
    max_hand_index: u32,
    calls: u32,
    riichi: u32,
    wins: u32,
}

pub async fn run_match(config: &RunConfig, variant: Variant) -> Result<RunReport, RunError> {
    let run_id = run_id()?;
    let mut bots = register_bots(config, variant, run_id).await?;
    let owner = &bots[0];
    let mut room = owner
        .client
        .create_room(&format!("{}牌效测试", variant.label()), variant)
        .await?;
    for bot in &bots[1..] {
        room = bot.client.join_room(&room.id, room.version).await?;
    }
    for bot in &bots {
        room = bot.client.set_ready(&room.id, room.version).await?;
    }
    for bot in &mut bots {
        bot.seat = room
            .members
            .iter()
            .find(|member| member.user_id == bot.user_id)
            .map(|member| member.seat)
            .ok_or_else(|| RunError::State("机器人没有房间座位".to_owned()))?;
    }
    let started = bots[0].client.start_room(&room.id, room.version).await?;
    let match_id = started.match_id;
    if !config.quiet {
        println!(
            "{}牌效测试开始 · 房间 {} · 对局 {}",
            variant.label(),
            room.id,
            match_id
        );
    }

    let mut view = bots[0].client.match_view(&match_id).await?;
    let mut stats = RunStats::default();
    loop {
        stats.max_hand_index = stats.max_hand_index.max(view.hand_index);
        if let Some(result) = view.result.clone() {
            return Ok(RunReport {
                variant,
                match_id,
                commands: stats.commands,
                hands: result.hand_count,
                calls: stats.calls,
                riichi: stats.riichi,
                wins: stats.wins,
                result,
            });
        }
        if stats.commands >= config.max_commands {
            return Err(RunError::State(format!(
                "超过最大命令数 {}，对局可能停滞",
                config.max_commands
            )));
        }

        view = match view.phase {
            MatchPhase::AwaitingTurnAction { seat } | MatchPhase::AwaitingDiscard { seat } => {
                let bot_index = bot_index(&bots, seat)?;
                let actor_view = bots[bot_index].client.match_view(&match_id).await?;
                let command =
                    strategy::turn_command(&actor_view, variant).map_err(RunError::Strategy)?;
                execute_turn(
                    &bots[bot_index],
                    &match_id,
                    actor_view,
                    command,
                    variant,
                    config,
                    &mut stats,
                )
                .await?
            }
            MatchPhase::AwaitingResponses { .. } => {
                execute_response(&bots, &match_id, variant, config, &mut stats).await?
            }
            MatchPhase::Ended { .. } => bots[0].client.match_view(&match_id).await?,
        };
    }
}

async fn register_bots(
    config: &RunConfig,
    variant: Variant,
    run_id: u64,
) -> Result<Vec<Bot>, RunError> {
    let mut bots = Vec::with_capacity(usize::from(variant.seat_count()));
    for index in 0..variant.seat_count() {
        let mut client = ApiClient::new(&config.server_url)?;
        let login_name = format!("bot_{run_id}_{index}");
        let nickname = format!("牌效机器人{}", index + 1);
        let auth = client.register(&login_name, &nickname).await?;
        bots.push(Bot {
            client,
            user_id: auth.user.id,
            seat: index,
        });
    }
    Ok(bots)
}

#[allow(clippy::too_many_arguments)]
async fn execute_turn(
    bot: &Bot,
    match_id: &str,
    view: MatchView,
    command: BotCommand,
    variant: Variant,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    let version = view.version;
    match submit(bot, match_id, version, &command, config, stats).await {
        Ok(view) => Ok(view),
        Err(RunError::Api(error))
            if error.is_invalid_command()
                && matches!(command.name, "riichi.tsumo" | "riichi.riichi_discard") =>
        {
            let current = bot.client.match_view(match_id).await?;
            let fallback =
                strategy::fallback_discard(&current, variant).map_err(RunError::Strategy)?;
            submit(bot, match_id, current.version, &fallback, config, stats).await
        }
        Err(error) => Err(error),
    }
}

async fn execute_response(
    bots: &[Bot],
    match_id: &str,
    variant: Variant,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    for bot in bots {
        let view = bot.client.match_view(match_id).await?;
        if view.available_reactions.is_empty() {
            continue;
        }
        let command = strategy::reaction_command(&view, variant)
            .map_err(RunError::Strategy)?
            .ok_or_else(|| RunError::State("响应窗口没有可执行动作".to_owned()))?;
        return submit(bot, match_id, view.version, &command, config, stats).await;
    }
    Err(RunError::State(
        "服务端停留在响应窗口，但所有玩家均无合法响应".to_owned(),
    ))
}

async fn submit(
    bot: &Bot,
    match_id: &str,
    expected_version: u64,
    command: &BotCommand,
    config: &RunConfig,
    stats: &mut RunStats,
) -> Result<MatchView, RunError> {
    let view = bot
        .client
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
        "riichi.chi" | "riichi.pon" | "riichi.open_kan"
    ) {
        stats.calls += 1;
    }
    if matches!(command.name, "riichi.tsumo" | "riichi.ron") {
        stats.wins += 1;
    }
    if !config.quiet {
        println!(
            "{}{}局 #{} {} {}",
            wind_name(&view.progress.round_wind),
            view.progress.round_number,
            stats.commands,
            view.observer().map_err(RunError::State)?.nickname,
            command.description
        );
    }
    Ok(view)
}

fn bot_index(bots: &[Bot], seat: u8) -> Result<usize, RunError> {
    bots.iter()
        .position(|bot| bot.seat == seat)
        .ok_or_else(|| RunError::State(format!("找不到 {} 家机器人", wind(seat))))
}

fn run_id() -> Result<u64, RunError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| RunError::State(error.to_string()))?
        .as_millis();
    let compact = millis % 1_000_000_000_000;
    let process = u128::from(std::process::id() % 10_000);
    u64::try_from(compact * 10_000 + process)
        .map_err(|_| RunError::State("测试运行 ID 溢出".to_owned()))
}

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
