mod mongodb;
mod mssql;
mod mysql;
mod policy_loader;
mod postgres;
mod redis;

use std::sync::Arc;

use anyhow::Result;
use audit::{logger::StdoutSink, AuditLogger};
use policy::PolicyEngine;
use secrets::SecretsClient;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

pub struct ProxyContext {
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditLogger>,
    pub secrets: Option<Arc<SecretsClient>>,
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

    // 정책 파일 로드 (POLICY_FILE 환경변수)
    match policy_loader::load_from_file(&policy) {
        Ok(n) if n > 0 => info!("정책 파일에서 {} 규칙 추가 로드", n),
        Ok(_) => {}
        Err(e) => warn!("정책 파일 로드 오류: {}", e),
    }

    info!("정책 엔진 초기화 완료 ({} 규칙)", policy.list_rules().len());

    // 감사 로거 초기화
    let audit_path = std::env::var("AUDIT_LOG_PATH")
        .unwrap_or_else(|_| "/var/log/ganji-dac/audit.jsonl".to_string());
    let audit = Arc::new(
        AuditLogger::new()
            .add_sink(StdoutSink)
            .add_sink(audit::logger::FileSink::new(&audit_path)),
    );

    // AWS Secrets Manager 초기화 (선택적)
    let secrets = match SecretsClient::from_env().await {
        Ok(client) => {
            info!("AWS Secrets Manager 연결 완료");
            Some(Arc::new(client))
        }
        Err(e) => {
            warn!("Secrets Manager 초기화 실패 (환경변수 방식으로 폴백): {}", e);
            None
        }
    };

    let ctx = Arc::new(ProxyContext {
        policy,
        audit,
        secrets,
    });

    // ── 프록시 리스너 동시 실행 ─────────────────────────

    let pg_ctx = ctx.clone();
    let pg = tokio::spawn(async move {
        if let Err(e) = postgres::run_proxy(pg_ctx, "0.0.0.0:15432").await {
            tracing::error!("PostgreSQL 프록시 오류: {}", e);
        }
    });

    let my_ctx = ctx.clone();
    let my = tokio::spawn(async move {
        if let Err(e) = mysql::run_proxy(my_ctx, "0.0.0.0:15306").await {
            tracing::error!("MySQL 프록시 오류: {}", e);
        }
    });

    let mongo_ctx = ctx.clone();
    let mongo = tokio::spawn(async move {
        if let Err(e) = mongodb::run_proxy(mongo_ctx, "0.0.0.0:15017").await {
            tracing::error!("MongoDB 프록시 오류: {}", e);
        }
    });

    let redis_ctx = ctx.clone();
    let redis = tokio::spawn(async move {
        if let Err(e) = redis::run_proxy(redis_ctx, "0.0.0.0:16379").await {
            tracing::error!("Redis 프록시 오류: {}", e);
        }
    });

    let mssql_ctx = ctx.clone();
    let mssql = tokio::spawn(async move {
        if let Err(e) = mssql::run_proxy(mssql_ctx, "0.0.0.0:11433").await {
            tracing::error!("MSSQL 프록시 오류: {}", e);
        }
    });

    info!("PostgreSQL 프록시 → :15432");
    info!("MySQL     프록시 → :15306");
    info!("MongoDB   프록시 → :15017");
    info!("Redis     프록시 → :16379");
    info!("MSSQL     프록시 → :11433");
    info!("모든 프록시 대기 중...");

    let _ = tokio::join!(pg, my, mongo, redis, mssql);
    Ok(())
}
