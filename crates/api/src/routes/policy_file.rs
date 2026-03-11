/// 정책 파일 관련 API 엔드포인트
///
/// POST /api/rules/reload  — POLICY_FILE 환경변수 경로에서 정책 다시 로드
/// GET  /api/rules/export  — 현재 규칙 전체를 policies.json 포맷으로 내보내기
use std::sync::Arc;

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use policy::AccessRule;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/rules/reload", post(reload_policy))
        .route("/api/rules/export", get(export_policy))
}

// ── POST /api/rules/reload ───────────────────────────────

/// POLICY_FILE 환경변수 경로의 JSON 파일에서 규칙을 다시 로드합니다.
/// 기존 규칙은 유지되며 파일의 규칙이 upsert(덮어쓰기) 됩니다.
async fn reload_policy(State(state): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let path = std::env::var("POLICY_FILE").unwrap_or_else(|_| String::new());

    if path.is_empty() {
        return Err(ApiError::BadRequest(
            "POLICY_FILE 환경변수가 설정되지 않았습니다.".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| {
        warn!(path = %path, error = %e, "정책 파일 읽기 실패");
        ApiError::Internal(anyhow!("정책 파일 읽기 실패: {}", e))
    })?;

    let policy_file: PolicyFileDto = serde_json::from_str(&content).map_err(|e| {
        ApiError::BadRequest(format!("정책 파일 파싱 오류: {}", e))
    })?;

    let rule_count = policy_file.rules.len();
    for rule in policy_file.rules {
        state.policy.upsert_rule(rule);
    }

    info!(
        path = %path,
        rules_loaded = rule_count,
        total_rules = state.policy.list_rules().len(),
        "정책 파일 리로드 완료"
    );

    Ok(Json(json!({
        "message": "정책 파일 리로드 완료",
        "path": path,
        "rules_loaded": rule_count,
        "total_rules": state.policy.list_rules().len(),
    })))
}

// ── GET /api/rules/export ────────────────────────────────

/// 현재 메모리에 로드된 규칙 전체를 policies.json 포맷으로 반환합니다.
async fn export_policy(State(state): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let rules = state.policy.list_rules();

    info!(rule_count = rules.len(), "정책 내보내기 요청");

    Ok(Json(json!({
        "version": "1",
        "description": "간지-DAC 정책 내보내기",
        "rule_count": rules.len(),
        "rules": rules,
    })))
}

// ── DTO ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct PolicyFileDto {
    #[serde(default)]
    #[allow(dead_code)]
    pub version: String,
    pub rules: Vec<AccessRule>,
}
