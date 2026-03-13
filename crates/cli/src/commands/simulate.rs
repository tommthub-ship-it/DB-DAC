use anyhow::Result;
use clap::Args;
use colored::Colorize;
use serde_json::{Value, json};

use crate::client::DacClient;

#[derive(Args)]
pub struct SimulateArgs {
    /// 클라이언트 IP
    #[arg(long, default_value = "127.0.0.1")]
    pub client_ip: String,

    /// DB 사용자
    #[arg(long)]
    pub db_user: String,

    /// DB 타입 (postgres, mysql, mongodb, redis, mssql)
    #[arg(long)]
    pub db_type: String,

    /// 대상 DB 이름
    #[arg(long)]
    pub target_db: String,

    /// 실행할 쿼리 (선택)
    #[arg(long)]
    pub query: Option<String>,
}

pub async fn run(client: &DacClient, args: SimulateArgs) -> Result<()> {
    let body = json!({
        "client_ip": args.client_ip,
        "db_user": args.db_user,
        "db_type": args.db_type,
        "target_db": args.target_db,
        "query": args.query,
    });

    let resp: Value = client.post("/api/audit/simulate", &body).await?;

    let allowed = resp["result"]["allowed"].as_bool().unwrap_or(false);
    let reason = resp["result"]["reason"].as_str().unwrap_or("-");
    let matched = resp["result"]["matched_rule"]
        .as_str()
        .map(|s| format!(" (규칙: {})", s.cyan()))
        .unwrap_or_default();

    let verdict = if allowed {
        "✓ ALLOW".green().bold().to_string()
    } else {
        "✗ DENY".red().bold().to_string()
    };

    println!("\n{}{}", verdict, matched);
    println!("  이유: {}", reason);
    println!();
    println!("요청 상세:");
    println!("  IP:       {}", args.client_ip.cyan());
    println!("  사용자:   {}", args.db_user.cyan());
    println!("  DB 타입:  {}", args.db_type.cyan());
    println!("  대상 DB:  {}", args.target_db.cyan());
    if let Some(q) = &args.query {
        println!("  쿼리:     {}", q.dimmed());
    }

    Ok(())
}
