mod api;
mod model;
mod runner;
mod strategy;

use std::error::Error;

use runner::{RunConfig, Variant};

#[derive(Clone, Copy)]
enum Selection {
    All,
    One(Variant),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (config, selection) = arguments()?;
    let variants: &[Variant] = match selection {
        Selection::All => &[Variant::Yonma, Variant::Sanma],
        Selection::One(Variant::Yonma) => &[Variant::Yonma],
        Selection::One(Variant::Sanma) => &[Variant::Sanma],
    };
    for variant in variants {
        let report = runner::run_match(&config, *variant).await?;
        println!("{} · 对局 {}", report.summary(), report.match_id);
    }
    Ok(())
}

fn arguments() -> Result<(RunConfig, Selection), String> {
    let mut config = RunConfig {
        server_url: std::env::var("MAMAHJONG_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned()),
        max_commands: 10_000,
        quiet: false,
    };
    let mut selection = Selection::All;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--server" => {
                config.server_url = args.next().ok_or_else(|| "--server 缺少地址".to_owned())?;
            }
            "--variant" => {
                selection = match args.next().as_deref() {
                    Some("yonma") => Selection::One(Variant::Yonma),
                    Some("sanma") => Selection::One(Variant::Sanma),
                    _ => return Err("--variant 只接受 yonma 或 sanma".to_owned()),
                };
            }
            "--all" => selection = Selection::All,
            "--max-commands" => {
                config.max_commands = args
                    .next()
                    .ok_or_else(|| "--max-commands 缺少数值".to_owned())?
                    .parse()
                    .map_err(|_| "--max-commands 必须是正整数".to_owned())?;
                if config.max_commands == 0 {
                    return Err("--max-commands 必须大于 0".to_owned());
                }
            }
            "--quiet" => config.quiet = true,
            "--help" | "-h" => {
                println!(
                    "用法：mamahjong-bot [--all | --variant yonma|sanma] \
                     [--server URL] [--max-commands N] [--quiet]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("未知参数：{argument}")),
        }
    }
    Ok((config, selection))
}
