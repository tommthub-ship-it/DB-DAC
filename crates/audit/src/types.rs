use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 감사 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// 접속 시도
    ConnectionAttempt,
    /// 접속 허용
    ConnectionAllowed,
    /// 접속 차단
    ConnectionDenied,
    /// 쿼리 실행
    QueryExecuted,
    /// 쿼리 차단
    QueryBlocked,
    /// 위험 쿼리 경고
    QueryAlert,
    /// 정책 변경
    PolicyChanged,
}

/// 감사 로그 이벤트 (개인정보보호법 §29 접근이력 관리)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,

    /// 접속자 정보
    pub client_ip: String,
    pub db_user: String,
    pub db_type: String,
    pub target_db: String,

    /// 쿼리 정보
    pub query: Option<String>,
    pub query_masked: bool, // 개인정보 마스킹 여부

    /// 판단 결과
    pub allowed: bool,
    pub reason: String,
    pub matched_rule: Option<String>,

    /// AWS 메타데이터
    pub aws_account_id: Option<String>,
    pub aws_region: Option<String>,
    pub iam_arn: Option<String>,
}

impl AuditEvent {
    pub fn new(
        event_type: EventType,
        client_ip: impl Into<String>,
        db_user: impl Into<String>,
        db_type: impl Into<String>,
        target_db: impl Into<String>,
        allowed: bool,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            client_ip: client_ip.into(),
            db_user: db_user.into(),
            db_type: db_type.into(),
            target_db: target_db.into(),
            query: None,
            query_masked: false,
            allowed,
            reason: reason.into(),
            matched_rule: None,
            aws_account_id: None,
            aws_region: None,
            iam_arn: None,
        }
    }
}
