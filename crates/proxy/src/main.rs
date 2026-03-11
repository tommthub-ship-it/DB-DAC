mod postgres;

use std::sync::Arc;

use anyhow::Result;
use audit::{AuditLogger, logger::StdoutSink};
use policy::PolicyEngine;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

pub struct ProxyContext {
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditLogger>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 로그 초기화
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .json()
        .init();

    info!("🔐 간지-DAC Proxy 시작");

    // 정책 엔진 초기화
    let policy = Arc::new(PolicyEngine::new());
    policy.load_default_rules();
    info!("정책 엔진 초기화 완료 ({} 규칙)", policy.list_rules().len());

    // 감사 로거 초기화
    let audit = Arc::new(AuditLogger::new().add_sink(StdoutSink));

    let ctx = Arc::new(ProxyContext { policy, audit });

    // PostgreSQL 프록시 시작
    let pg_ctx = ctx.clone();
    let pg_handle = tokio::spawn(async move {
        if let Err(e) = postgres::run_proxy(pg_ctx, "0.0.0.0:15432").await {
            tracing::error!("PostgreSQL 프록시 오류: {}", e);
        }
    });

    info!("PostgreSQL 프록시 → :15432");
    info!("모든 프록시 대기 중...");

    pg_handle.await?;
    Ok(())
}
