use chrono::{Datelike, Timelike, Utc, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::types::{AccessRequest, DbType};

/// 규칙 매칭 후 행동
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Deny,
    Alert, // 허용하되 알림
}

/// 접근 제어 규칙
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32, // 낮을수록 높은 우선순위
    pub action: RuleAction,
    pub enabled: bool,

    /// 조건들 (모두 AND)
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// IP 범위 (CIDR 지원)
    IpRange { cidr: String },

    /// DB 사용자
    DbUser { pattern: String },

    /// DB 타입
    DbType { db_type: DbType },

    /// 대상 DB 이름
    TargetDb { pattern: String },

    /// 쿼리 패턴 (정규식)
    QueryPattern { regex: String },

    /// 시간대 제한 (개인정보보호법 요건)
    TimeRange {
        start_hour: u32, // 0-23
        end_hour: u32,
        days: Vec<String>, // ["mon","tue","wed","thu","fri","sat","sun"]
    },

    /// IAM ARN 패턴
    IamArn { pattern: String },

    /// 위험 쿼리 차단 (DROP, TRUNCATE, DELETE without WHERE 등)
    BlockDangerousQuery,
}

impl Condition {
    pub fn matches(&self, req: &AccessRequest) -> bool {
        match self {
            Condition::IpRange { cidr } => {
                if let Ok(net) = cidr.parse::<ipnet::IpNet>() {
                    if let Ok(ip) = req.client_ip.parse::<std::net::IpAddr>() {
                        return net.contains(&ip);
                    }
                }
                false
            }

            Condition::DbUser { pattern } => {
                if let Ok(re) = Regex::new(pattern) {
                    re.is_match(&req.db_user)
                } else {
                    req.db_user == *pattern
                }
            }

            Condition::DbType { db_type } => &req.db_type == db_type,

            Condition::TargetDb { pattern } => {
                if let Ok(re) = Regex::new(pattern) {
                    re.is_match(&req.target_db)
                } else {
                    req.target_db == *pattern
                }
            }

            Condition::QueryPattern { regex } => {
                if let Some(query) = &req.query {
                    if let Ok(re) = Regex::new(regex) {
                        return re.is_match(query);
                    }
                }
                false
            }

            Condition::TimeRange {
                start_hour,
                end_hour,
                days,
            } => {
                let now = Utc::now();
                let hour = now.hour();
                let day = match now.weekday() {
                    Weekday::Mon => "mon",
                    Weekday::Tue => "tue",
                    Weekday::Wed => "wed",
                    Weekday::Thu => "thu",
                    Weekday::Fri => "fri",
                    Weekday::Sat => "sat",
                    Weekday::Sun => "sun",
                };

                let in_hours = if start_hour <= end_hour {
                    hour >= *start_hour && hour < *end_hour
                } else {
                    // 자정 넘어가는 경우 (예: 22 ~ 06)
                    hour >= *start_hour || hour < *end_hour
                };

                let in_days = days.is_empty() || days.iter().any(|d| d == day);
                in_hours && in_days
            }

            Condition::IamArn { pattern } => {
                if let Some(arn) = &req.iam_arn {
                    if let Ok(re) = Regex::new(pattern) {
                        return re.is_match(arn);
                    }
                    return arn == pattern;
                }
                false
            }

            Condition::BlockDangerousQuery => {
                if let Some(query) = &req.query {
                    let q = query.to_uppercase().trim().to_string();
                    // DROP, TRUNCATE, DELETE without WHERE, UPDATE without WHERE
                    if q.starts_with("DROP ")
                        || q.starts_with("TRUNCATE ")
                        || q.contains("XACT_ABORT")
                    {
                        return true;
                    }
                    // DELETE/UPDATE without WHERE
                    if (q.starts_with("DELETE ") || q.starts_with("UPDATE "))
                        && !q.contains("WHERE")
                    {
                        return true;
                    }
                }
                false
            }
        }
    }
}

impl AccessRule {
    pub fn matches(&self, req: &AccessRequest) -> bool {
        if !self.enabled {
            return false;
        }
        self.conditions.iter().all(|c| c.matches(req))
    }
}
