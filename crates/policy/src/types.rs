use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 지원 DB 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    PostgreSQL,
    MySQL,
    MongoDB,
    Redis,
    MSSQL,
}

impl std::fmt::Display for DbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbType::PostgreSQL => write!(f, "postgresql"),
            DbType::MySQL => write!(f, "mysql"),
            DbType::MongoDB => write!(f, "mongodb"),
            DbType::Redis => write!(f, "redis"),
            DbType::MSSQL => write!(f, "mssql"),
        }
    }
}

/// DB 접근 요청 컨텍스트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRequest {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,

    /// 접속자 정보
    pub client_ip: String,
    pub db_user: String,
    pub db_type: DbType,
    pub target_db: String,

    /// 쿼리 정보
    pub query: Option<String>,

    /// AWS 메타데이터
    pub aws_account_id: Option<String>,
    pub aws_region: Option<String>,
    pub iam_arn: Option<String>,
}

impl AccessRequest {
    pub fn new(
        client_ip: impl Into<String>,
        db_user: impl Into<String>,
        db_type: DbType,
        target_db: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            client_ip: client_ip.into(),
            db_user: db_user.into(),
            db_type,
            target_db: target_db.into(),
            query: None,
            aws_account_id: None,
            aws_region: None,
            iam_arn: None,
        }
    }
}

/// 정책 엔진 판단 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessResult {
    pub request_id: Uuid,
    pub allowed: bool,
    pub reason: String,
    pub matched_rule: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AccessResult {
    pub fn allow(request_id: Uuid, reason: impl Into<String>, rule: Option<String>) -> Self {
        Self {
            request_id,
            allowed: true,
            reason: reason.into(),
            matched_rule: rule,
            timestamp: Utc::now(),
        }
    }

    pub fn deny(request_id: Uuid, reason: impl Into<String>, rule: Option<String>) -> Self {
        Self {
            request_id,
            allowed: false,
            reason: reason.into(),
            matched_rule: rule,
            timestamp: Utc::now(),
        }
    }
}
