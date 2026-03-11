use anyhow::Result;
use async_trait::async_trait;
use tracing::{error, info};

use crate::types::AuditEvent;

/// 감사 로거 트레이트
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn write(&self, event: &AuditEvent) -> Result<()>;
}

/// 복합 감사 로거 (여러 싱크로 동시 전송)
pub struct AuditLogger {
    sinks: Vec<Box<dyn AuditSink>>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self { sinks: vec![] }
    }

    pub fn add_sink(mut self, sink: impl AuditSink + 'static) -> Self {
        self.sinks.push(Box::new(sink));
        self
    }

    pub async fn log(&self, event: AuditEvent) {
        for sink in &self.sinks {
            if let Err(e) = sink.write(&event).await {
                error!("감사 로그 전송 실패: {}", e);
            }
        }
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// 표준 출력 싱크 (개발/디버그용)
pub struct StdoutSink;

#[async_trait]
impl AuditSink for StdoutSink {
    async fn write(&self, event: &AuditEvent) -> Result<()> {
        let json = serde_json::to_string(event)?;
        info!(audit = %json, "감사 로그");
        Ok(())
    }
}

/// JSON 파일 싱크 (로컬 보관)
pub struct FileSink {
    path: std::path::PathBuf,
}

impl FileSink {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl AuditSink for FileSink {
    async fn write(&self, event: &AuditEvent) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let mut line = serde_json::to_string(event)?;
        line.push('\n');

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}

/// AWS CloudWatch Logs 싱크
pub struct CloudWatchSink {
    log_group: String,
    log_stream: String,
}

impl CloudWatchSink {
    pub fn new(log_group: impl Into<String>, log_stream: impl Into<String>) -> Self {
        Self {
            log_group: log_group.into(),
            log_stream: log_stream.into(),
        }
    }
}

#[async_trait]
impl AuditSink for CloudWatchSink {
    async fn write(&self, event: &AuditEvent) -> Result<()> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = aws_sdk_cloudwatchlogs::Client::new(&config);

        let message = serde_json::to_string(event)?;
        let timestamp = event.timestamp.timestamp_millis();

        client
            .put_log_events()
            .log_group_name(&self.log_group)
            .log_stream_name(&self.log_stream)
            .log_events(
                aws_sdk_cloudwatchlogs::types::InputLogEvent::builder()
                    .timestamp(timestamp)
                    .message(message)
                    .build()?,
            )
            .send()
            .await?;

        Ok(())
    }
}
