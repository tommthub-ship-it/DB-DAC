use std::sync::Arc;

use audit::{logger::StdoutSink, AuditLogger};
use policy::PolicyEngine;
use secrets::SecretsClient;
use tracing::warn;

#[allow(dead_code)]
pub struct AppState {
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditLogger>,
    pub secrets: Option<Arc<SecretsClient>>,
    pub api_key: String,
}

impl AppState {
    pub async fn new() -> Self {
        let api_key = std::env::var("API_SECRET")
            .unwrap_or_else(|_| "change-me-in-production".to_string());

        let audit = Arc::new(AuditLogger::new().add_sink(StdoutSink));

        let secrets = match SecretsClient::from_env().await {
            Ok(client) => Some(Arc::new(client)),
            Err(e) => {
                warn!("Secrets Manager 초기화 실패: {}", e);
                None
            }
        };

        Self {
            policy: Arc::new(PolicyEngine::new()),
            audit,
            secrets,
            api_key,
        }
    }
}
