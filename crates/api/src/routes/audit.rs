/// 감사 로그 조회 API
///
/// GET  /api/audit/simulate   접근 요청 시뮬레이션 (정책 테스트)
use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::post};
use policy::types::{AccessRequest, DbType};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/audit/simulate", post(simulate))
}

#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    pub client_ip: String,
    pub db_user: String,
    pub db_type: String,
    pub target_db: String,
    pub query: Option<String>,
}

/// 정책 시뮬레이션 — 실제 DB 연결 없이 정책 판단 결과 확인
async fn simulate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SimulateRequest>,
) -> ApiResult<Json<Value>> {
    let db_type = match body.db_type.to_lowercase().as_str() {
        "postgresql" | "postgres" | "pg" => DbType::PostgreSQL,
        "mysql" | "mariadb" => DbType::MySQL,
        "mongodb" | "mongo" => DbType::MongoDB,
        "redis" => DbType::Redis,
        "mssql" | "sqlserver" => DbType::MSSQL,
        other => {
            return Ok(Json(json!({
                "error": format!("지원하지 않는 DB 타입: {}", other)
            })));
        }
    };

    let mut req = AccessRequest::new(&body.client_ip, &body.db_user, db_type, &body.target_db);
    req.query = body.query.clone();

    let result = state.policy.evaluate(&req);

    Ok(Json(json!({
        "request": {
            "client_ip": body.client_ip,
            "db_user": body.db_user,
            "db_type": body.db_type,
            "target_db": body.target_db,
            "query": body.query,
        },
        "result": {
            "allowed": result.allowed,
            "reason": result.reason,
            "matched_rule": result.matched_rule,
        }
    })))
}
