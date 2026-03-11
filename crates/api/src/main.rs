mod auth;
mod error;
mod routes;
mod state;

use std::sync::Arc;

use anyhow::Result;
use axum::{Router, middleware};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .json()
        .init();

    info!("🔐 간지-DAC API 시작");

    let state = Arc::new(AppState::new().await);
    state.policy.load_default_rules();

    let app = Router::new()
        .merge(routes::rules::router())
        .merge(routes::policy_file::router())
        .merge(routes::audit::router())
        .merge(routes::secrets::router())
        .merge(routes::health::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_api_key,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = std::env::var("API_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Admin API → http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
