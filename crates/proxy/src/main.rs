mod mysql;
mod postgres;

use std::sync::Arc;

use anyhow::Result;
use audit::{logger::StdoutSink, AuditLogger};
use policy::PolicyEngine;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

pub struct ProxyContext {
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditLogger>,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .json()
        .init();

    info!("🔐 간지-DAC Proxy 시작");

    // 정책 엔진 초기화
    let policy = Arc::new(PolicyEngine::new());
    policy.load_default_rules();
    info!("정책 엔진 초기화 완료 ({} 규칙)", policy.list_rules().len());

    // 감사 로거 초기화 (파일 싱크 추가)
    let audit_path = std::env::var("AUDIT_LOG_PATH")
        .unwrap_or_else(|_| "/var/log/ganji-dac/audit.jsonl".to_string());

    let audit = Arc::new(
        AuditLogger::new()
            .add_sink(StdoutSink)
            .add_sink(audit::logger::FileSink::new(&audit_path)),
    );

    let ctx = Arc::new(ProxyContext { policy, audit });

    // ── 프록시 리스너 동시 실행 ─────────────────────────

    // PostgreSQL
    let pg_ctx = ctx.clone();
    let pg = tokio::spawn(async move {
        if let Err(e) = postgres::run_proxy(pg_ctx, "0.0.0.0:15432").await {
            tracing::error!("PostgreSQL 프록시 오류: {}", e);
        }
    });

    // MySQL
    let my_ctx = ctx.clone();
    let my = tokio::spawn(async move {
        if let Err(e) = mysql::run_proxy(my_ctx, "0.0.0.0:15306").await {
            tracing::error!("MySQL 프록시 오류: {}", e);
        }
    });

    info!("PostgreSQL 프록시 → :15432");
    info!("MySQL     프록시 → :15306");
    info!("모든 프록시 대기 중...");

    let _ = tokio::join!(pg, my);
    Ok(())
}
