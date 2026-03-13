mod client;
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::client::DacClient;
use crate::commands::{health, rules, secrets, simulate};

/// 🔐 간지-DAC Admin CLI
#[derive(Parser)]
#[command(
    name = "dac",
    version,
    about = "간지-DAC DB 접근제어 관리 CLI",
    long_about = None,
)]
struct Cli {
    /// Admin API URL
    #[arg(
        long,
        env = "DAC_URL",
        default_value = "http://localhost:8080",
        global = true
    )]
    url: String,

    /// API 인증 키
    #[arg(
        long,
        env = "DAC_API_KEY",
        default_value = "",
        global = true,
        hide_env_values = true
    )]
    key: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 서버 상태 확인
    Health,

    /// 접근 정책 규칙 관리
    #[command(subcommand)]
    Rules(rules::RulesCmd),

    /// 정책 시뮬레이션 (실제 DB 연결 없이 판단 결과 확인)
    Simulate(simulate::SimulateArgs),

    /// AWS Secrets Manager 자격증명 관리
    #[command(subcommand)]
    Secrets(secrets::SecretsCmd),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{} {}", "오류:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let client = DacClient::new(&cli.url, &cli.key)?;

    match cli.command {
        Command::Health => health::run(&client).await,
        Command::Rules(cmd) => rules::run(&client, cmd).await,
        Command::Simulate(args) => simulate::run(&client, args).await,
        Command::Secrets(cmd) => secrets::run(&client, cmd).await,
    }
}
