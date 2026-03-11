use std::sync::Arc;

use audit::{logger::StdoutSink, AuditLogger};
use policy::PolicyEngine;

#[allow(dead_code)]
pub struct AppState {
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditLogger>,
    pub api_key: String,
}

impl AppState {
    pub fn new() -> Self {
        let api_key = std::env::var("API_SECRET")
            .unwrap_or_else(|_| "change-me-in-production".to_string());

        let audit = Arc::new(AuditLogger::new().add_sink(StdoutSink));

        Self {
            policy: Arc::new(PolicyEngine::new()),
            audit,
            api_key,
        }
    }
}
