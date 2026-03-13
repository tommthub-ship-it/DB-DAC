use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::Value;
use tabled::{Table, Tabled, settings::Style};

use crate::client::DacClient;

#[derive(Subcommand)]
pub enum SecretsCmd {
    /// 자격증명 조회 (비밀번호 마스킹)
    Get {
        /// Secrets Manager 시크릿 ID (ARN 또는 이름)
        secret_id: String,
    },

    /// 캐시 무효화 + 강제 재조회 (로테이션 직후 사용)
    Refresh {
        /// 시크릿 ID
        secret_id: String,
    },

    /// 로테이션 상태 확인
    Rotation {
        /// 시크릿 ID
        secret_id: String,
    },
}

#[derive(Tabled)]
struct SecretRow {
    #[tabled(rename = "항목")]
    key: String,
    #[tabled(rename = "값")]
    value: String,
}

pub async fn run(client: &DacClient, cmd: SecretsCmd) -> Result<()> {
    match cmd {
        SecretsCmd::Get { secret_id } => get(client, &secret_id).await,
        SecretsCmd::Refresh { secret_id } => refresh(client, &secret_id).await,
        SecretsCmd::Rotation { secret_id } => rotation(client, &secret_id).await,
    }
}

async fn get(client: &DacClient, secret_id: &str) -> Result<()> {
    let resp: Value = client
        .get_raw(&format!("/api/secrets/{}", urlencoded(secret_id)))
        .await?;

    let rows = vec![
        SecretRow { key: "Secret ID".into(),  value: resp["secret_id"].as_str().unwrap_or("-").into() },
        SecretRow { key: "사용자".into(),      value: resp["username"].as_str().unwrap_or("-").into() },
        SecretRow { key: "비밀번호".into(),    value: "********".dimmed().to_string() },
        SecretRow { key: "엔진".into(),        value: resp["engine"].as_str().unwrap_or("-").cyan().to_string() },
        SecretRow { key: "호스트".into(),      value: resp["host"].as_str().unwrap_or("-").into() },
        SecretRow { key: "포트".into(),        value: resp["port"].as_u64().map(|p| p.to_string()).unwrap_or_else(|| "-".into()) },
        SecretRow { key: "DB 이름".into(),     value: resp["db_name"].as_str().unwrap_or("-").into() },
    ];

    let table = Table::new(rows).with(Style::rounded()).to_string();
    println!("{}", table);
    Ok(())
}

async fn refresh(client: &DacClient, secret_id: &str) -> Result<()> {
    let resp: Value = client
        .post_empty(&format!("/api/secrets/{}/refresh", urlencoded(secret_id)))
        .await?;
    println!(
        "{} {}",
        "✓".green().bold(),
        resp["message"].as_str().unwrap_or("갱신 완료"),
    );
    Ok(())
}

async fn rotation(client: &DacClient, secret_id: &str) -> Result<()> {
    let resp: Value = client
        .get_raw(&format!("/api/secrets/{}/rotation", urlencoded(secret_id)))
        .await?;

    let status = resp["rotation_status"].as_str().unwrap_or("Unknown");
    let colored_status = match status {
        "Stable" => status.green().to_string(),
        "Rotating" => status.yellow().bold().to_string(),
        _ => status.into(),
    };

    println!("Secret ID:  {}", secret_id.cyan());
    println!("로테이션:   {}", colored_status);
    Ok(())
}

/// 시크릿 ID에 슬래시가 있을 수 있으므로 percent-encode
fn urlencoded(s: &str) -> String {
    // ARN에 포함된 ':' '/' 등을 그대로 넘겨도 reqwest가 처리하지만
    // 명시적으로 path segment로만 쓸 땐 encode 권장
    s.replace('/', "%2F").replace(':', "%3A")
}
