use std::sync::Arc;

use dashmap::DashMap;
use tracing::{info, warn};

use crate::{
    rule::{AccessRule, RuleAction},
    types::{AccessRequest, AccessResult},
};

/// 정책 엔진 — 규칙 기반 접근제어 판단
pub struct PolicyEngine {
    rules: Arc<DashMap<String, AccessRule>>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(DashMap::new()),
        }
    }

    /// 규칙 추가/갱신
    pub fn upsert_rule(&self, rule: AccessRule) {
        info!(rule_id = %rule.id, rule_name = %rule.name, "정책 규칙 등록");
        self.rules.insert(rule.id.clone(), rule);
    }

    /// 규칙 삭제
    pub fn remove_rule(&self, rule_id: &str) -> bool {
        self.rules.remove(rule_id).is_some()
    }

    /// 전체 규칙 조회
    pub fn list_rules(&self) -> Vec<AccessRule> {
        let mut rules: Vec<AccessRule> = self.rules.iter().map(|r| r.value().clone()).collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// 접근 요청 평가 (개인정보보호법 §29: 접근통제)
    pub fn evaluate(&self, req: &AccessRequest) -> AccessResult {
        let mut sorted_rules: Vec<AccessRule> =
            self.rules.iter().map(|r| r.value().clone()).collect();
        sorted_rules.sort_by_key(|r| r.priority);

        for rule in &sorted_rules {
            if rule.matches(req) {
                match rule.action {
                    RuleAction::Allow => {
                        info!(
                            request_id = %req.id,
                            client_ip = %req.client_ip,
                            db_user = %req.db_user,
                            rule = %rule.id,
                            "접근 허용"
                        );
                        return AccessResult::allow(req.id, &rule.name, Some(rule.id.clone()));
                    }
                    RuleAction::Deny => {
                        warn!(
                            request_id = %req.id,
                            client_ip = %req.client_ip,
                            db_user = %req.db_user,
                            rule = %rule.id,
                            "접근 차단"
                        );
                        return AccessResult::deny(req.id, &rule.name, Some(rule.id.clone()));
                    }
                    RuleAction::Alert => {
                        warn!(
                            request_id = %req.id,
                            client_ip = %req.client_ip,
                            db_user = %req.db_user,
                            rule = %rule.id,
                            "접근 허용 (경고)"
                        );
                        return AccessResult::allow(
                            req.id,
                            format!("[ALERT] {}", rule.name),
                            Some(rule.id.clone()),
                        );
                    }
                }
            }
        }

        // 기본 정책: 명시적 허용 없으면 차단 (Deny-by-default)
        warn!(
            request_id = %req.id,
            client_ip = %req.client_ip,
            db_user = %req.db_user,
            "기본 정책: 접근 차단 (매칭 규칙 없음)"
        );
        AccessResult::deny(req.id, "기본 차단 정책 (매칭 규칙 없음)", None)
    }

    /// 기본 규칙 셋 로드 (대한민국 개인정보보호법 기준)
    pub fn load_default_rules(&self) {
        use crate::rule::Condition;

        // 1. 위험 쿼리 전면 차단
        self.upsert_rule(AccessRule {
            id: "default-block-dangerous".to_string(),
            name: "위험 쿼리 차단 (DROP/TRUNCATE/조건없는 DELETE)".to_string(),
            description: Some("개인정보보호법 §29 기술적 보호조치".to_string()),
            priority: 1,
            action: RuleAction::Deny,
            enabled: true,
            conditions: vec![Condition::BlockDangerousQuery],
        });

        // 2. 업무 시간 외 접근 경고 (평일 09-18시 외)
        self.upsert_rule(AccessRule {
            id: "default-offhours-alert".to_string(),
            name: "업무 시간 외 접근 경고".to_string(),
            description: Some("ISMS-P 접근이력 관리".to_string()),
            priority: 10,
            action: RuleAction::Alert,
            enabled: true,
            conditions: vec![
                // 업무시간 외 = 18시~09시
                Condition::TimeRange {
                    start_hour: 18,
                    end_hour: 9,
                    days: vec![
                        "mon".to_string(),
                        "tue".to_string(),
                        "wed".to_string(),
                        "thu".to_string(),
                        "fri".to_string(),
                    ],
                },
            ],
        });
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}
