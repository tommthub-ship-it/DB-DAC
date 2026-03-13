use anyhow::{Context, Result, bail};
use clap::Subcommand;
use colored::Colorize;
use serde_json::Value;
use tabled::{Table, Tabled, settings::Style};

use crate::client::DacClient;

#[derive(Subcommand)]
pub enum RulesCmd {
    /// 전체 규칙 목록 조회
    List,

    /// 규칙 단건 조회
    Get {
        /// 규칙 ID
        id: String,
    },

    /// 규칙 추가 (JSON 파일 또는 stdin)
    Add {
        /// JSON 파일 경로 (생략 시 stdin)
        #[arg(short, long)]
        file: Option<String>,

        /// 인라인 JSON 문자열
        #[arg(short, long)]
        json: Option<String>,
    },

    /// 규칙 수정
    Update {
        /// 규칙 ID
        id: String,

        /// JSON 파일 경로 (생략 시 stdin)
        #[arg(short, long)]
        file: Option<String>,

        /// 인라인 JSON 문자열
        #[arg(short, long)]
        json: Option<String>,
    },

    /// 규칙 삭제
    Delete {
        /// 규칙 ID
        id: String,
    },

    /// 규칙 활성화/비활성화 토글
    Toggle {
        /// 규칙 ID
        id: String,
    },

    /// POLICY_FILE에서 정책 다시 로드
    Reload,

    /// 현재 규칙 전체 내보내기 (JSON)
    Export {
        /// 저장할 파일 경로 (생략 시 stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

// ── 테이블 행 ─────────────────────────────────────────────

#[derive(Tabled)]
struct RuleRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "이름")]
    name: String,
    #[tabled(rename = "우선순위")]
    priority: String,
    #[tabled(rename = "액션")]
    action: String,
    #[tabled(rename = "활성")]
    enabled: String,
    #[tabled(rename = "조건 수")]
    conditions: String,
}

pub async fn run(client: &DacClient, cmd: RulesCmd) -> Result<()> {
    match cmd {
        RulesCmd::List => list(client).await,
        RulesCmd::Get { id } => get(client, &id).await,
        RulesCmd::Add { file, json } => add(client, file, json).await,
        RulesCmd::Update { id, file, json } => update(client, &id, file, json).await,
        RulesCmd::Delete { id } => delete(client, &id).await,
        RulesCmd::Toggle { id } => toggle(client, &id).await,
        RulesCmd::Reload => reload(client).await,
        RulesCmd::Export { output } => export(client, output).await,
    }
}

// ── list ──────────────────────────────────────────────────

async fn list(client: &DacClient) -> Result<()> {
    let resp: Value = client.get_raw("/api/rules").await?;
    let total = resp["total"].as_u64().unwrap_or(0);
    let rules = resp["rules"].as_array().cloned().unwrap_or_default();

    if rules.is_empty() {
        println!("{}", "규칙이 없습니다.".yellow());
        return Ok(());
    }

    let rows: Vec<RuleRow> = rules
        .iter()
        .map(|r| {
            let action = r["action"].as_str().unwrap_or("-").to_string();
            let action_colored = match action.as_str() {
                "allow" => action.green().to_string(),
                "deny" => action.red().to_string(),
                "alert" => action.yellow().to_string(),
                _ => action,
            };
            let enabled = if r["enabled"].as_bool().unwrap_or(false) {
                "✓".green().to_string()
            } else {
                "✗".red().to_string()
            };
            let cond_count = r["conditions"].as_array().map(|a| a.len()).unwrap_or(0);

            RuleRow {
                id: r["id"].as_str().unwrap_or("-").to_string(),
                name: r["name"].as_str().unwrap_or("-").to_string(),
                priority: r["priority"].as_i64().unwrap_or(0).to_string(),
                action: action_colored,
                enabled,
                conditions: cond_count.to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).with(Style::rounded()).to_string();
    println!("{}", table);
    println!("총 {} 개 규칙", total.to_string().bold());
    Ok(())
}

// ── get ───────────────────────────────────────────────────

async fn get(client: &DacClient, id: &str) -> Result<()> {
    let rule: Value = client.get_raw(&format!("/api/rules/{}", id)).await?;
    println!("{}", serde_json::to_string_pretty(&rule)?);
    Ok(())
}

// ── add ───────────────────────────────────────────────────

async fn add(client: &DacClient, file: Option<String>, json: Option<String>) -> Result<()> {
    let body = load_json(file, json)?;
    let resp: Value = client.post("/api/rules", &body).await?;
    println!("{} {}", "✓".green().bold(), resp["message"].as_str().unwrap_or("규칙 생성 완료"));
    if let Some(id) = resp["id"].as_str() {
        println!("  ID: {}", id.cyan());
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────

async fn update(client: &DacClient, id: &str, file: Option<String>, json: Option<String>) -> Result<()> {
    let body = load_json(file, json)?;
    let resp: Value = client.put(&format!("/api/rules/{}", id), &body).await?;
    println!("{} {}", "✓".green().bold(), resp["message"].as_str().unwrap_or("규칙 수정 완료"));
    Ok(())
}

// ── delete ────────────────────────────────────────────────

async fn delete(client: &DacClient, id: &str) -> Result<()> {
    let resp: Value = client.delete(&format!("/api/rules/{}", id)).await?;
    println!("{} {}", "✓".green().bold(), resp["message"].as_str().unwrap_or("규칙 삭제 완료"));
    Ok(())
}

// ── toggle ────────────────────────────────────────────────

async fn toggle(client: &DacClient, id: &str) -> Result<()> {
    let resp: Value = client.post_empty(&format!("/api/rules/{}/toggle", id)).await?;
    let enabled = resp["enabled"].as_bool().unwrap_or(false);
    let status = if enabled {
        "활성화".green()
    } else {
        "비활성화".yellow()
    };
    println!(
        "{} 규칙 '{}' {}",
        "✓".green().bold(),
        id.cyan(),
        status,
    );
    Ok(())
}

// ── reload ────────────────────────────────────────────────

async fn reload(client: &DacClient) -> Result<()> {
    let resp: Value = client.post_empty("/api/rules/reload").await?;
    println!("{} {}", "✓".green().bold(), resp["message"].as_str().unwrap_or("리로드 완료"));
    println!(
        "  경로: {}  로드: {}건  전체: {}건",
        resp["path"].as_str().unwrap_or("-").cyan(),
        resp["rules_loaded"].as_u64().unwrap_or(0).to_string().yellow(),
        resp["total_rules"].as_u64().unwrap_or(0).to_string().bold(),
    );
    Ok(())
}

// ── export ────────────────────────────────────────────────

async fn export(client: &DacClient, output: Option<String>) -> Result<()> {
    let resp: Value = client.get_raw("/api/rules/export").await?;
    let pretty = serde_json::to_string_pretty(&resp)?;

    match output {
        Some(path) => {
            std::fs::write(&path, &pretty).with_context(|| format!("파일 쓰기 실패: {}", path))?;
            println!(
                "{} {} 에 내보내기 완료 ({}건)",
                "✓".green().bold(),
                path.cyan(),
                resp["rule_count"].as_u64().unwrap_or(0),
            );
        }
        None => println!("{}", pretty),
    }
    Ok(())
}

// ── 헬퍼: JSON 로드 ──────────────────────────────────────

fn load_json(file: Option<String>, json: Option<String>) -> Result<Value> {
    if let Some(j) = json {
        return serde_json::from_str(&j).context("--json 파싱 실패");
    }
    if let Some(path) = file {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("파일 읽기 실패: {}", path))?;
        return serde_json::from_str(&content).context("파일 JSON 파싱 실패");
    }
    // stdin
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).context("stdin 읽기 실패")?;
    if buf.trim().is_empty() {
        bail!("JSON 입력이 없습니다. --file, --json, 또는 stdin을 사용하세요.");
    }
    serde_json::from_str(&buf).context("stdin JSON 파싱 실패")
}
