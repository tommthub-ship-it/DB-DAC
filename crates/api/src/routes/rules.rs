/// 접근 정책 규칙 CRUD API
///
/// GET    /api/rules          전체 규칙 조회
/// POST   /api/rules          규칙 생성
/// GET    /api/rules/:id      규칙 단건 조회
/// PUT    /api/rules/:id      규칙 수정
/// DELETE /api/rules/:id      규칙 삭제
/// POST   /api/rules/:id/toggle  활성화/비활성화 토글
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use policy::rule::AccessRule;
use serde_json::{Value, json};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/rules", get(list_rules).post(create_rule))
        .route("/api/rules/:id", get(get_rule).put(update_rule).delete(delete_rule))
        .route("/api/rules/:id/toggle", post(toggle_rule))
}

// ── 전체 규칙 조회 ──────────────────────────────────────

async fn list_rules(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rules = state.policy.list_rules();
    Json(json!({
        "total": rules.len(),
        "rules": rules,
    }))
}

// ── 규칙 단건 조회 ──────────────────────────────────────

async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<AccessRule>> {
    state
        .policy
        .list_rules()
        .into_iter()
        .find(|r| r.id == id)
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("규칙 '{}' 없음", id)))
}

// ── 규칙 생성 ────────────────────────────────────────────

async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<AccessRule>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    // ID 중복 체크
    if state.policy.list_rules().iter().any(|r| r.id == rule.id) {
        return Err(ApiError::BadRequest(format!(
            "규칙 ID '{}' 이미 존재",
            rule.id
        )));
    }

    let id = rule.id.clone();
    state.policy.upsert_rule(rule);

    tracing::info!(rule_id = %id, "규칙 생성");
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "message": "규칙 생성 완료" }))))
}

// ── 규칙 수정 ────────────────────────────────────────────

async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut rule): Json<AccessRule>,
) -> ApiResult<Json<Value>> {
    // 존재 여부 확인
    if !state.policy.list_rules().iter().any(|r| r.id == id) {
        return Err(ApiError::NotFound(format!("규칙 '{}' 없음", id)));
    }

    rule.id = id.clone(); // URL path ID 우선
    state.policy.upsert_rule(rule);

    tracing::info!(rule_id = %id, "규칙 수정");
    Ok(Json(json!({ "id": id, "message": "규칙 수정 완료" })))
}

// ── 규칙 삭제 ────────────────────────────────────────────

async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    if state.policy.remove_rule(&id) {
        tracing::info!(rule_id = %id, "규칙 삭제");
        Ok(Json(json!({ "id": id, "message": "규칙 삭제 완료" })))
    } else {
        Err(ApiError::NotFound(format!("규칙 '{}' 없음", id)))
    }
}

// ── 활성화/비활성화 토글 ─────────────────────────────────

async fn toggle_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let mut rule = state
        .policy
        .list_rules()
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("규칙 '{}' 없음", id)))?;

    rule.enabled = !rule.enabled;
    let enabled = rule.enabled;
    state.policy.upsert_rule(rule);

    tracing::info!(rule_id = %id, enabled = %enabled, "규칙 상태 변경");
    Ok(Json(json!({
        "id": id,
        "enabled": enabled,
        "message": if enabled { "규칙 활성화" } else { "규칙 비활성화" },
    })))
}
