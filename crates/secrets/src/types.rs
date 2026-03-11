use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// AWS Secrets Manager RDS 자격증명 포맷
/// https://docs.aws.amazon.com/secretsmanager/latest/userguide/reference_secret_json_structure.html
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCredential {
    pub username: String,
    pub password: String,

    /// DB 엔진 (mysql, postgres, sqlserver 등)
    #[serde(default)]
    pub engine: Option<String>,

    /// 실제 DB 호스트 (Secrets Manager에 저장된 경우)
    #[serde(default)]
    pub host: Option<String>,

    /// DB 포트
    #[serde(default)]
    pub port: Option<u16>,

    /// 기본 DB명
    #[serde(rename = "dbname", default)]
    pub db_name: Option<String>,
}

impl DbCredential {
    /// upstream 연결 주소 반환 (host:port)
    pub fn upstream_addr(&self, default_port: u16) -> Option<String> {
        self.host.as_ref().map(|h| {
            let port = self.port.unwrap_or(default_port);
            format!("{}:{}", h, port)
        })
    }
}

/// 캐시된 자격증명 래퍼
#[derive(Debug, Clone)]
pub struct CachedCredential {
    pub credential: DbCredential,
    pub secret_arn: String,
    pub version_id: String,
    pub cached_at: DateTime<Utc>,
    pub ttl_seconds: u64,
}

impl CachedCredential {
    pub fn is_expired(&self) -> bool {
        let age = Utc::now()
            .signed_duration_since(self.cached_at)
            .num_seconds();
        age as u64 >= self.ttl_seconds
    }
}

/// 로테이션 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RotationStatus {
    /// 로테이션 없음 또는 완료
    Stable,
    /// 로테이션 진행 중
    Rotating,
    /// 로테이션 실패
    Failed(String),
}
