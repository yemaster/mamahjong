mod api;
mod model;
mod runner;
mod strategy;

use std::error::Error;
use std::io::Write;

use colored::Colorize;
use model::RoomView;
use runner::{RunConfig, RunStats, Variant};

const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (config, _variant_hint) = parse_args()?;

    println!();
    println!(
        "{}",
        "  🀫  麻将自动打牌机器人  🀫  "
            .black()
            .on_bright_yellow()
            .bold()
    );
    println!();

    // --- 1. Server URL ---
    let server_url = if config.run.server_url != DEFAULT_SERVER_URL
        || std::env::var("MAMAHJONG_SERVER_URL").is_ok()
    {
        println!("{} {}", "● 服务器:".dimmed(), config.run.server_url.cyan());
        config.run.server_url.clone()
    } else {
        print!(
            "{} [{}]: ",
            "● 服务器地址".dimmed(),
            config.run.server_url.cyan()
        );
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            config.run.server_url.clone()
        } else {
            trimmed.to_owned()
        }
    };

    let mut client = api::ApiClient::new(&server_url)?;

    // --- 2. Login ---
    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        // Auto-login mode
        match client.login(username, password).await {
            Ok(auth) => {
                let nickname = auth.user.nickname().to_owned();
                println!(
                    "{} {}",
                    "✓".green().bold(),
                    format!("已登录: {}", nickname).green()
                );
            }
            Err(error) => {
                return Err(format!("自动登录失败: {error}").into());
            }
        }
    } else {
        let nickname = login_loop(&mut client).await?;
        println!(
            "{} {}",
            "✓".green().bold(),
            format!("已登录: {}", nickname).green()
        );
    }
    println!();

    // --- 3. Room join loop ---
    loop {
        let room = if let Some(room_id) = &config.room_id {
            // Auto-join mode
            join_room_direct(&client, room_id).await?
        } else {
            join_room_loop(&client).await?
        };

        // Wait for game start or cancel
        match wait_for_game(&client, &room, config.run.quiet).await {
            Ok(Some(match_id)) => {
                println!();
                println!(
                    "{} {}",
                    "▶".bright_yellow().bold(),
                    format!("游戏开始！对局: {}", match_id).bright_yellow()
                );
                println!();

                // --- 4. Display initial state ---
                let view = client.match_view(&match_id).await?;
                if let Err(e) = runner::display_initial_state(&view) {
                    println!("{} {}", "⚠".yellow(), format!("显示手牌失败: {e}").yellow());
                }

                // --- 5. Auto-play ---
                let mut stats = RunStats::default();
                let variant = runner::detect_variant(&view);
                match runner::auto_play(&client, &match_id, &config.run, &mut stats).await {
                    Ok(result) => {
                        let report = runner::RunReport {
                            variant,
                            commands: stats.commands,
                            hands: result.hand_count,
                            calls: stats.calls,
                            riichi: stats.riichi,
                            wins: stats.wins,
                            result,
                        };
                        println!();
                        println!("{}", "▔".repeat(40).dimmed());
                        println!("{}", report.summary().bold());
                        println!("{}", "▁".repeat(40).dimmed());

                        // If auto-mode, exit after game ends
                        if config.room_id.is_some() {
                            break;
                        }
                        break;
                    }
                    Err(error) => {
                        println!(
                            "{} {}",
                            "✗".red().bold(),
                            format!("自动打牌出错: {error}").red()
                        );
                        break;
                    }
                }
            }
            Ok(None) => {
                // User cancelled, loop back
                continue;
            }
            Err(error) => {
                println!(
                    "{} {}",
                    "✗".red().bold(),
                    format!("等待游戏开始出错: {error}").red()
                );
                continue;
            }
        }
    }

    Ok(())
}

async fn login_loop(client: &mut api::ApiClient) -> Result<String, Box<dyn Error>> {
    loop {
        print!("{} ", "登录名:".dimmed());
        std::io::stdout().flush()?;
        let mut login_name = String::new();
        std::io::stdin().read_line(&mut login_name)?;
        let login_name = login_name.trim().to_owned();
        if login_name.is_empty() {
            println!("{} {}", "✗".red(), "登录名不能为空".red());
            continue;
        }

        let password = rpassword::prompt_password(format!("{} ", "密  码:".dimmed()))?;
        if password.is_empty() {
            println!("{} {}", "✗".red(), "密码不能为空".red());
            continue;
        }

        match client.login(&login_name, &password).await {
            Ok(auth) => return Ok(auth.user.nickname().to_owned()),
            Err(error) => {
                println!(
                    "{} {}",
                    "✗".red().bold(),
                    format!("登录失败: {error}").red()
                );
                println!("{}", "请重试".dimmed());
            }
        }
    }
}

async fn join_room_loop(client: &api::ApiClient) -> Result<RoomView, Box<dyn Error>> {
    loop {
        print!("{} ", "房间号 (q 退出):".dimmed());
        std::io::stdout().flush()?;
        let mut room_id = String::new();
        std::io::stdin().read_line(&mut room_id)?;
        let room_id = room_id.trim().to_owned();
        if room_id.is_empty() {
            continue;
        }
        if room_id.eq_ignore_ascii_case("q") {
            println!("{}", "再见！".dimmed());
            std::process::exit(0);
        }

        match join_room_direct(client, &room_id).await {
            Ok(room) => return Ok(room),
            Err(error) => {
                println!("{} {}", "✗".red(), format!("加入失败: {error}").red());
                continue;
            }
        }
    }
}

/// Join a room directly without prompting, used for both interactive and auto mode.
async fn join_room_direct(
    client: &api::ApiClient,
    room_id: &str,
) -> Result<RoomView, Box<dyn Error>> {
    // Fetch current room state to get the correct version
    let room_info = match client.room(room_id, 0).await {
        Ok(room) => {
            println!(
                "  {} 「{}」· {}人 · {}",
                "房间".dimmed(),
                room.name.bold(),
                room.members.len(),
                match room.lifecycle.as_str() {
                    "waiting" => "等待中".green(),
                    "playing" => "游戏中".yellow(),
                    _ => room.lifecycle.dimmed(),
                }
            );
            room
        }
        Err(error) => {
            return Err(format!("获取房间信息失败: {error}").into());
        }
    };

    // Join with correct version
    let room = match client.join_room(room_id, room_info.version).await {
        Ok(room) => {
            println!(
                "  {} {} {}人",
                "✓".green(),
                "已加入".green(),
                room.members.len()
            );
            room
        }
        Err(error) if error.is_already_member() => {
            println!("  {} {}", "✓".green(), "已在房间中".green());
            room_info
        }
        Err(error) => {
            return Err(format!("加入失败: {error}").into());
        }
    };

    // Auto-ready
    match client.set_ready(room_id, room.version).await {
        Ok(room) => {
            println!("  {} {}", "✓".green(), "已准备".green());
            Ok(room)
        }
        Err(error) => {
            let _ = client.leave_room(room_id, room.version).await;
            Err(format!("准备失败: {error}").into())
        }
    }
}

/// Wait for the game to start. Returns `Ok(Some(match_id))` when the game starts,
/// `Ok(None)` if the user cancels.
async fn wait_for_game(
    client: &api::ApiClient,
    room: &RoomView,
    quiet: bool,
) -> Result<Option<String>, Box<dyn Error>> {
    use tokio::io::AsyncBufReadExt;

    let room_id = room.id.clone();
    let mut version = room.version;

    print!("{}", "  等待游戏开始".dimmed());
    std::io::stdout().flush()?;

    let mut stdin_lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut dots: u8 = 0;

    // In headless/auto mode stdin is at EOF, so `next_line()` resolves with
    // `Ok(None)` immediately every iteration.  Without guarding against that,
    // the select below would spin on the always-ready stdin branch and never
    // poll the room.  Once stdin reports EOF (or a read error), stop watching
    // it and just poll the room on the sleep timer.
    let stdin_closed = std::sync::atomic::AtomicBool::new(false);

    loop {
        tokio::select! {
            stdin_result = async {
                if stdin_closed.load(std::sync::atomic::Ordering::Relaxed) {
                    std::future::pending::<std::io::Result<Option<String>>>().await
                } else {
                    stdin_lines.next_line().await
                }
            } => {
                match stdin_result {
                    Ok(Some(line)) if line.trim().eq_ignore_ascii_case("c") => {
                        println!();
                        println!("  {} {}", "↩".yellow(), "取消等待，离开房间...".yellow());
                        match client.leave_room(&room_id, version).await {
                            Ok(_) => println!("  {} {}", "✓".green(), "已离开房间".green()),
                            Err(e) => println!("  {} {}", "⚠".yellow(), format!("离开失败: {e}").yellow()),
                        }
                        println!();
                        return Ok(None);
                    }
                    Ok(None) | Err(_) => {
                        // EOF or read error: disable the cancel-input feature.
                        stdin_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    _ => {
                        // Redraw the waiting prompt after stray input
                        print!("\r\x1b[K{}", "  等待游戏开始".dimmed());
                        std::io::stdout().flush()?;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                // Animated dots
                dots = (dots + 1) % 4;
                let dot_str = ".".repeat(dots as usize);
                print!("\r\x1b[K{}{} {}", "  等待游戏开始".dimmed(), dot_str.dimmed(), "[c 取消]".dimmed());
                std::io::stdout().flush()?;

                // Poll room state
                match client.room(&room_id, version).await {
                    Ok(room) => {
                        version = room.version;
                        if room.lifecycle == "playing" {
                            print!("\r\x1b[K");
                            if let Some(match_id) = room.active_match_id {
                                return Ok(Some(match_id));
                            }
                            if !quiet {
                                eprintln!("{} {}", "⚠".yellow(), "房间状态异常，继续等待...".yellow());
                            }
                        }
                    }
                    Err(error) => {
                        if !quiet {
                            // Don't spam, just note once in a while
                            if dots == 0 {
                                eprint!("\r\x1b[K{} {}", "⚠".yellow(), format!("轮询错误: {error}").yellow());
                                std::io::stdout().flush()?;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Bot configuration with optional auto-login.
#[derive(Debug)]
struct BotConfig {
    run: RunConfig,
    username: Option<String>,
    password: Option<String>,
    room_id: Option<String>,
}

/// CLI argument parsing.
fn parse_args() -> Result<(BotConfig, Option<Variant>), String> {
    let mut config = BotConfig {
        run: RunConfig {
            server_url: std::env::var("MAMAHJONG_SERVER_URL")
                .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned()),
            max_commands: 10_000,
            quiet: false,
        },
        username: None,
        password: None,
        room_id: None,
    };
    let mut variant: Option<Variant> = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--server" => {
                config.run.server_url =
                    args.next().ok_or_else(|| "--server 缺少地址".to_owned())?;
            }
            "--variant" => {
                variant = match args.next().as_deref() {
                    Some("yonma") => Some(Variant::Yonma),
                    Some("sanma") => Some(Variant::Sanma),
                    Some("impact") => Some(Variant::Impact),
                    Some("sichuan") => Some(Variant::Sichuan),
                    _ => return Err("--variant 只接受 yonma / sanma / impact / sichuan".to_owned()),
                };
            }
            "--username" | "-u" => {
                config.username = Some(
                    args.next()
                        .ok_or_else(|| "--username 缺少用户名".to_owned())?,
                );
            }
            "--password" | "-p" => {
                config.password = Some(
                    args.next()
                        .ok_or_else(|| "--password 缺少密码".to_owned())?,
                );
            }
            "--room" | "-r" => {
                config.room_id = Some(args.next().ok_or_else(|| "--room 缺少房间号".to_owned())?);
            }
            "--quiet" => config.run.quiet = true,
            "--help" | "-h" => {
                println!("用法：mamahjong-bot [选项]");
                println!();
                println!("选项：");
                println!(
                    "  --server URL          服务器地址 (默认: {})",
                    DEFAULT_SERVER_URL
                );
                println!("  --variant TYPE        游戏变体 (yonma/sanma/impact/sichuan)");
                println!("  -u, --username NAME   自动登录用户名");
                println!("  -p, --password PASS   自动登录密码");
                println!("  -r, --room ID         自动加入房间号");
                println!("  --quiet               安静模式");
                println!("  -h, --help            显示帮助");
                std::process::exit(0);
            }
            _ => return Err(format!("未知参数：{argument}")),
        }
    }
    Ok((config, variant))
}
