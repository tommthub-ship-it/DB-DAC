/// Redis RESP 프록시
/// 클라이언트 → 간지-DAC → 실제 Redis
///
/// RESP(REdis Serialization Protocol) 파싱:
///   *N\r\n          — N개 요소의 배열
///   $N\r\n          — N바이트 Bulk String
///   +OK\r\n         — Simple String
///   -ERR msg\r\n    — Error
///   :1234\r\n       — Integer
use std::sync::Arc;

use anyhow::Result;
use audit::types::{AuditEvent, EventType};
use policy::types::{AccessRequest, DbType};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};
use tracing::{error, info, warn};

use crate::ProxyContext;

/// 차단 시 클라이언트에 반환하는 RESP 에러 응답
const DENY_RESP: &[u8] = b"-ERR Access denied by ganji-DAC\r\n";

/// 위험 커맨드 목록 (대소문자 무관)
const DANGEROUS_COMMANDS: &[&str] = &["FLUSHALL", "FLUSHDB", "CONFIG", "DEBUG", "SHUTDOWN", "SLAVEOF", "REPLICAOF"];

fn upstream_addr() -> String {
    std::env::var("REDIS_UPSTREAM").unwrap_or_else(|_| "127.0.0.1:6379".to_string())
}

pub async fn run_proxy(ctx: Arc<ProxyContext>, listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    info!("Redis 프록시 수신 대기: {}", listen_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        let ctx = ctx.clone();
        let client_ip = client_addr.ip().to_string();

        tokio::spawn(async move {
            info!(client_ip = %client_ip, "새 Redis 연결");
            if let Err(e) = handle_connection(ctx, client_stream, client_ip).await {
                error!("Redis 연결 처리 오류: {}", e);
            }
        });
    }
}

async fn handle_connection(
    ctx: Arc<ProxyContext>,
    client: TcpStream,
    client_ip: String,
) -> Result<()> {
    let upstream_addr = upstream_addr();

    let upstream = match TcpStream::connect(&upstream_addr).await {
        Ok(s) => s,
        Err(e) => {
            error!("Redis 업스트림 연결 실패 {}: {}", upstream_addr, e);
            return Err(e.into());
        }
    };

    // 연결 레벨 정책 평가
    let req = AccessRequest::new(&client_ip, "unknown", DbType::Redis, "redis");
    let result = ctx.policy.evaluate(&req);

    let mut conn_event = AuditEvent::new(
        if result.allowed {
            EventType::ConnectionAllowed
        } else {
            EventType::ConnectionDenied
        },
        &client_ip,
        "unknown",
        "redis",
        "redis",
        result.allowed,
        &result.reason,
    );
    conn_event.matched_rule = result.matched_rule.clone();
    ctx.audit.log(conn_event).await;

    if !result.allowed {
        warn!(client_ip = %client_ip, "Redis 연결 차단: {}", result.reason);
        // Redis에는 연결 레벨 에러가 없으므로 소켓 닫기
        return Ok(());
    }

    info!(client_ip = %client_ip, "Redis 연결 터널링 시작");
    tunnel_with_inspection(ctx, client, upstream, client_ip).await
}

/// 양방향 터널 (RESP 커맨드 인터셉트)
async fn tunnel_with_inspection(
    ctx: Arc<ProxyContext>,
    client: TcpStream,
    upstream: TcpStream,
    client_ip: String,
) -> Result<()> {
    let (client_read, client_write_half) = client.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    // c2u(에러 응답)와 u2c(업스트림 응답 포워딩) 양쪽에서 사용
    let client_write = Arc::new(AsyncMutex::new(client_write_half));
    let client_write_c = client_write.clone();
    let client_write_u = client_write.clone();

    let ctx_c2u = ctx.clone();
    let client_ip_c = client_ip.clone();

    // 클라이언트 → 업스트림 (커맨드 인터셉트)
    let c2u = tokio::spawn(async move {
        let mut reader = BufReader::new(client_read);

        loop {
            // RESP 커맨드 읽기 (전체 raw bytes 보존)
            let (command, raw_bytes) = match read_resp_command(&mut reader).await {
                Ok(Some(r)) => r,
                Ok(None) => break, // EOF
                Err(e) => {
                    error!("RESP 파싱 오류: {}", e);
                    break;
                }
            };

            let cmd_upper = command.to_uppercase();

            // 위험 커맨드 감지
            let is_dangerous = DANGEROUS_COMMANDS.contains(&cmd_upper.as_str());

            // 정책 평가
            let mut req = AccessRequest::new(&client_ip_c, "unknown", DbType::Redis, "redis");
            req.query = Some(cmd_upper.clone());
            let result = ctx_c2u.policy.evaluate(&req);

            let blocked = !result.allowed || is_dangerous;

            let event_type = if blocked {
                EventType::QueryBlocked
            } else {
                EventType::QueryExecuted
            };

            let block_reason = if is_dangerous && result.allowed {
                format!("위험 커맨드 차단: {}", cmd_upper)
            } else {
                result.reason.clone()
            };

            let mut event = AuditEvent::new(
                event_type,
                &client_ip_c,
                "unknown",
                "redis",
                "redis",
                !blocked,
                if blocked { &block_reason } else { &result.reason },
            );
            event.query = Some(cmd_upper.clone());
            event.matched_rule = result.matched_rule.clone();
            ctx_c2u.audit.log(event).await;

            if blocked {
                warn!(command = %cmd_upper, client_ip = %client_ip_c, "Redis 커맨드 차단");
                if client_write_c.lock().await.write_all(DENY_RESP).await.is_err() {
                    break;
                }
                continue;
            }

            if upstream_write.write_all(&raw_bytes).await.is_err() {
                break;
            }
        }
    });

    // 업스트림 → 클라이언트 (패스스루)
    let u2c = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            match upstream_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if client_write_u.lock().await.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let _ = tokio::join!(c2u, u2c);
    Ok(())
}

// ── RESP 파서 ────────────────────────────────────────────

/// RESP 커맨드를 읽어서 (커맨드 이름, 원본 바이트) 반환
///
/// Array 형식(*N\r\n$len\r\nbytes\r\n...)과
/// Inline 형식(CMD arg1 arg2\r\n) 모두 지원
async fn read_resp_command<R>(reader: &mut BufReader<R>) -> Result<Option<(String, Vec<u8>)>>
where
    R: AsyncReadExt + Unpin,
{
    let mut raw = Vec::new();
    let mut first_line = String::new();

    let n = reader.read_line(&mut first_line).await?;
    if n == 0 {
        return Ok(None); // EOF
    }

    raw.extend_from_slice(first_line.as_bytes());

    let first_line = first_line.trim_end_matches("\r\n").trim_end_matches('\n');

    if first_line.is_empty() {
        return Ok(Some((String::new(), raw)));
    }

    // Array 형식
    if let Some(count_str) = first_line.strip_prefix('*') {
        let count: usize = count_str.parse().unwrap_or(0);
        let mut command_name = String::new();

        for i in 0..count {
            // $N 라인
            let mut bulk_header = String::new();
            let bh_n = reader.read_line(&mut bulk_header).await?;
            if bh_n == 0 {
                return Ok(None);
            }
            raw.extend_from_slice(bulk_header.as_bytes());

            let bulk_header_trim = bulk_header.trim_end_matches("\r\n").trim_end_matches('\n');
            let bulk_len: usize = if let Some(s) = bulk_header_trim.strip_prefix('$') {
                s.parse().unwrap_or(0)
            } else {
                0
            };

            // 데이터 + \r\n
            let mut bulk_data = vec![0u8; bulk_len + 2];
            reader.read_exact(&mut bulk_data).await?;
            raw.extend_from_slice(&bulk_data);

            // 첫 번째 인자 = 커맨드 이름
            if i == 0 {
                command_name =
                    String::from_utf8_lossy(&bulk_data[..bulk_len]).to_string();
            }
        }

        return Ok(Some((command_name, raw)));
    }

    // Inline 커맨드 형식 (예: PING, QUIT)
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let command_name = parts.first().map(|s| s.to_string()).unwrap_or_default();
    Ok(Some((command_name, raw)))
}
