/// 감사 로그 조회 API
///
/// GET  /api/audit/simulate   접근 요청 시뮬레이션 (정책 테스트)
/// GET  /api/audit/logs       감사 로그 목록 조회
use std::sync::Arc;

use axum::{Json, Router, extract::{Query, State}, routing::{get, post}};
use policy::types::{AccessRequest, DbType};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/audit/simulate", post(simulate))
        .route("/api/audit/logs", get(logs))
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

/// 감사 로그 조회 쿼리 파라미터
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub event_type: Option<String>,
    pub db_user: Option<String>,
    pub client_ip: Option<String>,
    pub allowed: Option<bool>,
}

/// 감사 로그 목록 조회
/// GET /api/audit/logs
pub async fn logs(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<LogsQuery>,
) -> ApiResult<Json<Value>> {
    let log_path = std::env::var("AUDIT_LOG_PATH")
        .unwrap_or_else(|_| "/tmp/ganji-audit.jsonl".to_string());

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    // 파일을 줄별로 읽어 AuditEvent 파싱
    let mut events: Vec<audit::types::AuditEvent> = Vec::new();

    if let Ok(file) = tokio::fs::File::open(&log_path).await {
        let reader = tokio::io::BufReader::new(file);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<audit::types::AuditEvent>(&line) {
                events.push(event);
            }
        }
    }

    // 필터링
    let filtered: Vec<&audit::types::AuditEvent> = events.iter().filter(|e| {
        // event_type 필터
        if let Some(ref et) = params.event_type {
            let et_str = serde_json::to_string(&e.event_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            if !et_str.eq_ignore_ascii_case(et) {
                return false;
            }
        }
        // db_user 부분매칭
        if let Some(ref du) = params.db_user {
            if !e.db_user.to_lowercase().contains(&du.to_lowercase()) {
                return false;
            }
        }
        // client_ip 부분매칭
        if let Some(ref ip) = params.client_ip {
            if !e.client_ip.contains(ip.as_str()) {
                return false;
            }
        }
        // allowed 필터
        if let Some(al) = params.allowed {
            if e.allowed != al {
                return false;
            }
        }
        true
    }).collect();

    let total = filtered.len();
    let has_more = offset + limit < total;

    let page: Vec<Value> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
        .collect();

    Ok(Json(json!({
        "total": total,
        "logs": page,
        "has_more": has_more,
    })))
}
