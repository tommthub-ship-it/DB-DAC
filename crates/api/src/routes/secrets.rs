/// AWS Secrets Manager 관리 API
///
/// GET    /api/secrets/:id            자격증명 조회 (비밀번호 마스킹)
/// POST   /api/secrets/:id/refresh    캐시 무효화 + 강제 재조회
/// GET    /api/secrets/:id/rotation   로테이션 상태 확인
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/secrets/:id", get(get_secret))
        .route("/api/secrets/:id/refresh", post(refresh_secret))
        .route("/api/secrets/:id/rotation", get(rotation_status))
}

/// 자격증명 조회 (비밀번호 마스킹 — 감사 목적으로만)
async fn get_secret(
    State(state): State<Arc<AppState>>,
    Path(secret_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let client = state
        .secrets
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Secrets Manager 미연결".to_string()))?;

    let cred = client
        .get_db_credential(&secret_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // 비밀번호는 절대 노출하지 않음
    Ok(Json(json!({
        "secret_id": secret_id,
        "username": cred.username,
        "password": "********",
        "engine": cred.engine,
        "host": cred.host,
        "port": cred.port,
        "db_name": cred.db_name,
    })))
}

/// 캐시 무효화 + 강제 재조회 (로테이션 직후 수동 갱신)
async fn refresh_secret(
    State(state): State<Arc<AppState>>,
    Path(secret_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let client = state
        .secrets
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Secrets Manager 미연결".to_string()))?;

    client
        .refresh_credential(&secret_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    tracing::info!(secret_id = %secret_id, "자격증명 강제 갱신");
    Ok(Json(json!({
        "secret_id": secret_id,
        "message": "자격증명 갱신 완료",
    })))
}

/// 로테이션 상태 확인
async fn rotation_status(
    State(state): State<Arc<AppState>>,
    Path(secret_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let client = state
        .secrets
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Secrets Manager 미연결".to_string()))?;

    let status = client
        .rotation_status(&secret_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(Json(json!({
        "secret_id": secret_id,
        "rotation_status": format!("{:?}", status),
    })))
}
