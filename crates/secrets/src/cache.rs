use dashmap::DashMap;
use tracing::{debug, info};

use crate::types::CachedCredential;

/// 인메모리 자격증명 캐시
/// - TTL 기반 자동 만료
/// - 로테이션 감지 시 즉시 무효화
pub struct CredentialCache {
    store: DashMap<String, CachedCredential>,
    default_ttl: u64,
}

impl CredentialCache {
    pub fn new(default_ttl_seconds: u64) -> Self {
        Self {
            store: DashMap::new(),
            default_ttl: default_ttl_seconds,
        }
    }

    /// 캐시에서 자격증명 조회 (만료 시 None 반환)
    pub fn get(&self, secret_id: &str) -> Option<CachedCredential> {
        let entry = self.store.get(secret_id)?;
        if entry.is_expired() {
            debug!(secret_id = %secret_id, "자격증명 캐시 만료");
            drop(entry);
            self.store.remove(secret_id);
            None
        } else {
            debug!(secret_id = %secret_id, "자격증명 캐시 히트");
            Some(entry.clone())
        }
    }

    /// 캐시 저장
    pub fn set(&self, secret_id: &str, mut cred: CachedCredential) {
        if cred.ttl_seconds == 0 {
            cred.ttl_seconds = self.default_ttl;
        }
        info!(
            secret_id = %secret_id,
            ttl = %cred.ttl_seconds,
            "자격증명 캐시 저장"
        );
        self.store.insert(secret_id.to_string(), cred);
    }

    /// 특정 시크릿 캐시 무효화 (로테이션 감지 시 호출)
    pub fn invalidate(&self, secret_id: &str) {
        if self.store.remove(secret_id).is_some() {
            info!(secret_id = %secret_id, "자격증명 캐시 무효화");
        }
    }

    /// 전체 캐시 무효화
    pub fn invalidate_all(&self) {
        self.store.clear();
        info!("전체 자격증명 캐시 무효화");
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }
}
