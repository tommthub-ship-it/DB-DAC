use std::sync::Arc;

use anyhow::Result;
use aws_sdk_secretsmanager::Client;
use chrono::Utc;
use tracing::{info, warn};

use crate::{
    cache::CredentialCache,
    error::SecretsError,
    types::{CachedCredential, DbCredential, RotationStatus},
};

/// AWS Secrets Manager 클라이언트
/// - 자격증명 조회 + 캐싱 + 로테이션 감지
pub struct SecretsClient {
    client: Client,
    cache: Arc<CredentialCache>,
    /// 캐시 TTL (초) — 기본 300s (5분)
    cache_ttl: u64,
}

impl SecretsClient {
    /// 환경에서 AWS 설정 자동 로드 (IAM Role / 환경변수 / ~/.aws)
    pub async fn from_env() -> Result<Self> {
        let config =
            aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Ok(Self {
            client,
            cache: Arc::new(CredentialCache::new(300)),
            cache_ttl: 300,
        })
    }

    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.cache_ttl = ttl_seconds;
        self
    }

    /// DB 자격증명 조회 (캐시 우선, 만료 시 AWS에서 재조회)
    pub async fn get_db_credential(&self, secret_id: &str) -> Result<DbCredential, SecretsError> {
        // 1. 캐시 확인
        if let Some(cached) = self.cache.get(secret_id) {
            return Ok(cached.credential);
        }

        // 2. AWS에서 조회
        info!(secret_id = %secret_id, "Secrets Manager에서 자격증명 조회");
        self.fetch_and_cache(secret_id).await
    }

    /// AWS에서 직접 조회 (캐시 무시 — 로테이션 직후 사용)
    pub async fn refresh_credential(&self, secret_id: &str) -> Result<DbCredential, SecretsError> {
        self.cache.invalidate(secret_id);
        self.fetch_and_cache(secret_id).await
    }

    /// 로테이션 상태 확인
    pub async fn rotation_status(&self, secret_id: &str) -> Result<RotationStatus, SecretsError> {
        let resp = self
            .client
            .describe_secret()
            .secret_id(secret_id)
            .send()
            .await
            .map_err(|e| SecretsError::AwsError(e.to_string()))?;

        // 로테이션 중 = LastRotatedDate가 없거나 RotationEnabled + 최근 변경
        let rotating = resp.rotation_enabled().unwrap_or(false)
            && resp.last_rotated_date().is_none();

        if rotating {
            warn!(secret_id = %secret_id, "자격증명 로테이션 진행 중");
            Ok(RotationStatus::Rotating)
        } else {
            Ok(RotationStatus::Stable)
        }
    }

    /// 로테이션 감지 후 자동 갱신 (백그라운드 태스크용)
    pub async fn watch_rotation(&self, secret_id: &str) -> Result<bool, SecretsError> {
        let status = self.rotation_status(secret_id).await?;
        if status == RotationStatus::Rotating {
            // 로테이션 완료 대기 후 캐시 무효화
            self.cache.invalidate(secret_id);
            info!(secret_id = %secret_id, "로테이션 감지 — 캐시 무효화");
            return Ok(true);
        }
        Ok(false)
    }

    /// 캐시 강제 무효화 (수동 갱신 API용)
    pub fn invalidate_cache(&self, secret_id: &str) {
        self.cache.invalidate(secret_id);
    }

    // ── 내부 헬퍼 ──────────────────────────────────────

    async fn fetch_and_cache(&self, secret_id: &str) -> Result<DbCredential, SecretsError> {
        let resp = self
            .client
            .get_secret_value()
            .secret_id(secret_id)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("ResourceNotFoundException") {
                    SecretsError::NotFound(secret_id.to_string())
                } else {
                    SecretsError::AwsError(msg)
                }
            })?;

        let secret_string = resp
            .secret_string()
            .ok_or_else(|| SecretsError::ParseError("SecretString 없음 (바이너리 시크릿은 미지원)".to_string()))?;

        let credential: DbCredential = serde_json::from_str(secret_string)
            .map_err(|e| SecretsError::ParseError(e.to_string()))?;

        let version_id = resp
            .version_id()
            .unwrap_or("unknown")
            .to_string();

        let secret_arn = resp
            .arn()
            .unwrap_or(secret_id)
            .to_string();

        // 캐시 저장
        self.cache.set(
            secret_id,
            CachedCredential {
                credential: credential.clone(),
                secret_arn,
                version_id,
                cached_at: Utc::now(),
                ttl_seconds: self.cache_ttl,
            },
        );

        info!(
            secret_id = %secret_id,
            engine = ?credential.engine,
            host = ?credential.host,
            "자격증명 조회 완료"
        );

        Ok(credential)
    }
}
