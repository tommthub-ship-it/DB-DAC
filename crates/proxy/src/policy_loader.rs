/// 정책 파일 로더
///
/// POLICY_FILE 환경변수로 지정한 JSON 파일에서 정책 규칙을 로드합니다.
/// config/policies.json 포맷 사용:
///
/// ```json
/// {
///   "version": "1",
///   "rules": [ { "id": "...", "name": "...", ... } ]
/// }
/// ```
///
/// 파일이 없거나 환경변수가 설정되지 않으면 기본 정책만 사용합니다.
use std::sync::Arc;

use anyhow::Result;
use policy::{AccessRule, PolicyEngine};
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct PolicyFile {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub rules: Vec<AccessRule>,
}

/// POLICY_FILE 환경변수 경로에서 정책을 읽어 엔진에 로드
///
/// 기본 규칙은 이미 로드된 상태에서 추가로 파일 규칙을 적재합니다.
/// 동일 ID 규칙은 파일 내용으로 덮어씁니다(upsert).
pub fn load_from_file(engine: &Arc<PolicyEngine>) -> Result<usize> {
    let path = match std::env::var("POLICY_FILE") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            info!("POLICY_FILE 환경변수 미설정, 기본 정책만 사용");
            return Ok(0);
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            warn!("정책 파일 읽기 실패 '{}': {} — 기본 정책만 사용", path, e);
            return Ok(0);
        }
    };

    let policy_file: PolicyFile = serde_json::from_str(&content).map_err(|e| {
        anyhow::anyhow!("정책 파일 파싱 오류 '{}': {}", path, e)
    })?;

    let rule_count = policy_file.rules.len();

    for rule in policy_file.rules {
        engine.upsert_rule(rule);
    }

    info!(
        path = %path,
        version = %policy_file.version,
        description = ?policy_file.description,
        rules_loaded = rule_count,
        "정책 파일 로드 완료"
    );

    Ok(rule_count)
}

/// 현재 엔진 규칙을 PolicyFile JSON 포맷으로 직렬화
#[allow(dead_code)]
pub fn export_to_json(engine: &Arc<PolicyEngine>) -> Result<String> {
    let rules = engine.list_rules();

    let policy_file = serde_json::json!({
        "version": "1",
        "description": "간지-DAC 정책 내보내기",
        "rules": rules,
    });

    Ok(serde_json::to_string_pretty(&policy_file)?)
}
