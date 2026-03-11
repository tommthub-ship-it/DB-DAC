use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("시크릿 '{0}' 없음")]
    NotFound(String),

    #[error("자격증명 파싱 실패: {0}")]
    ParseError(String),

    #[error("AWS API 오류: {0}")]
    AwsError(String),

    #[error("캐시 오류: {0}")]
    CacheError(String),
}
