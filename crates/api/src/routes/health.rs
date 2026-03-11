use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

pub fn router() -> Router<std::sync::Arc<crate::state::AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "간지-DAC",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
