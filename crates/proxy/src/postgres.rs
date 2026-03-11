/// PostgreSQL 프로토콜 프록시
/// 클라이언트 → 간지-DAC → 실제 PostgreSQL
use std::sync::Arc;

use anyhow::Result;
use audit::types::{AuditEvent, EventType};
use bytes::{BufMut, Bytes, BytesMut};
use policy::types::{AccessRequest, DbType};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

use crate::ProxyContext;

/// 환경변수에서 실제 PG 주소 읽기
fn upstream_addr() -> String {
    std::env::var("PG_UPSTREAM").unwrap_or_else(|_| "127.0.0.1:5432".to_string())
}

pub async fn run_proxy(ctx: Arc<ProxyContext>, listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    info!("PostgreSQL 프록시 수신 대기: {}", listen_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        let ctx = ctx.clone();
        let client_ip = client_addr.ip().to_string();

        tokio::spawn(async move {
            info!(client_ip = %client_ip, "새 PostgreSQL 연결");
            if let Err(e) = handle_connection(ctx, client_stream, client_ip).await {
                error!("연결 처리 오류: {}", e);
            }
        });
    }
}

async fn handle_connection(
    ctx: Arc<ProxyContext>,
    mut client: TcpStream,
    client_ip: String,
) -> Result<()> {
    // 1. PostgreSQL Startup 메시지 읽기
    let startup = read_startup_message(&mut client).await?;
    let db_user = startup.user.clone().unwrap_or_default();
    let db_name = startup.database.clone().unwrap_or_default();

    // 2. 정책 평가
    let req = AccessRequest::new(&client_ip, &db_user, DbType::PostgreSQL, &db_name);
    let result = ctx.policy.evaluate(&req);

    // 3. 감사 로그
    let event_type = if result.allowed {
        EventType::ConnectionAllowed
    } else {
        EventType::ConnectionDenied
    };

    let mut audit_event = AuditEvent::new(
        event_type,
        &client_ip,
        &db_user,
        "postgresql",
        &db_name,
        result.allowed,
        &result.reason,
    );
    audit_event.matched_rule = result.matched_rule.clone();
    ctx.audit.log(audit_event).await;

    // 4. 차단 처리
    if !result.allowed {
        warn!(
            client_ip = %client_ip,
            db_user = %db_user,
            "연결 차단: {}",
            result.reason
        );
        send_pg_error(&mut client, &result.reason).await?;
        return Ok(());
    }

    // 5. 업스트림 연결
    let upstream_addr = upstream_addr();
    let mut upstream = match TcpStream::connect(&upstream_addr).await {
        Ok(s) => s,
        Err(e) => {
            error!("업스트림 연결 실패 {}: {}", upstream_addr, e);
            send_pg_error(&mut client, "DB 서버에 연결할 수 없습니다").await?;
            return Err(e.into());
        }
    };

    // Startup 메시지를 업스트림으로 전달
    upstream.write_all(&startup.raw).await?;

    info!(
        client_ip = %client_ip,
        db_user = %db_user,
        db_name = %db_name,
        "연결 터널링 시작"
    );

    // 6. 양방향 터널 (쿼리 인터셉트 포함)
    tunnel_with_inspection(ctx, client, upstream, client_ip, db_user, db_name).await?;

    Ok(())
}

/// 양방향 데이터 터널 (쿼리 감사 포함)
async fn tunnel_with_inspection(
    ctx: Arc<ProxyContext>,
    client: TcpStream,
    upstream: TcpStream,
    client_ip: String,
    db_user: String,
    db_name: String,
) -> Result<()> {
    let (mut client_read, mut client_write) = client.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    let ctx_c2u = ctx.clone();
    let client_ip_c = client_ip.clone();
    let db_user_c = db_user.clone();
    let db_name_c = db_name.clone();

    // 클라이언트 → 업스트림 (쿼리 인터셉트)
    let c2u = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            match client_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = &buf[..n];

                    // PostgreSQL 쿼리 메시지 파싱 (Simple Query: 'Q')
                    if data[0] == b'Q' && n > 5 {
                        if let Ok(query) = std::str::from_utf8(&data[5..n - 1]) {
                            let mut req = AccessRequest::new(
                                &client_ip_c,
                                &db_user_c,
                                DbType::PostgreSQL,
                                &db_name_c,
                            );
                            req.query = Some(query.to_string());
                            let result = ctx_c2u.policy.evaluate(&req);

                            let event_type = if result.allowed {
                                EventType::QueryExecuted
                            } else {
                                EventType::QueryBlocked
                            };

                            let mut event = AuditEvent::new(
                                event_type,
                                &client_ip_c,
                                &db_user_c,
                                "postgresql",
                                &db_name_c,
                                result.allowed,
                                &result.reason,
                            );
                            event.query = Some(query.to_string());
                            event.matched_rule = result.matched_rule.clone();
                            ctx_c2u.audit.log(event).await;

                            if !result.allowed {
                                warn!(query = %query, "쿼리 차단");
                                // 차단된 쿼리는 업스트림으로 전달하지 않음
                                // TODO: 클라이언트에 에러 응답 전송
                                continue;
                            }
                        }
                    }

                    if upstream_write.write_all(data).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 업스트림 → 클라이언트 (그대로 전달)
    let u2c = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            match upstream_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if client_write.write_all(&buf[..n]).await.is_err() {
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

/// PostgreSQL Startup 메시지
struct StartupMessage {
    user: Option<String>,
    database: Option<String>,
    raw: Bytes,
}

async fn read_startup_message(stream: &mut TcpStream) -> Result<StartupMessage> {
    // 길이 읽기 (4바이트)
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // 나머지 읽기
    let mut body = vec![0u8; len - 4];
    stream.read_exact(&mut body).await?;

    let mut raw = BytesMut::new();
    raw.put_slice(&len_buf);
    raw.put_slice(&body);

    // 파라미터 파싱 (protocol version 이후)
    let mut user = None;
    let mut database = None;

    if body.len() > 4 {
        let params = &body[4..]; // protocol version 스킵
        let mut parts = params.split(|&b| b == 0);
        while let (Some(key), Some(val)) = (parts.next(), parts.next()) {
            if key.is_empty() {
                break;
            }
            let k = String::from_utf8_lossy(key);
            let v = String::from_utf8_lossy(val);
            match k.as_ref() {
                "user" => user = Some(v.to_string()),
                "database" => database = Some(v.to_string()),
                _ => {}
            }
        }
    }

    Ok(StartupMessage {
        user,
        database,
        raw: raw.freeze(),
    })
}

/// 클라이언트에 PostgreSQL 오류 전송
async fn send_pg_error(stream: &mut TcpStream, message: &str) -> Result<()> {
    let mut buf = BytesMut::new();
    // ErrorResponse ('E')
    let msg = format!("SFATAL\0VFATAL\0C28000\0M{}\0\0", message);
    let len = (msg.len() + 4) as u32;
    buf.put_u8(b'E');
    buf.put_u32(len);
    buf.put_slice(msg.as_bytes());
    stream.write_all(&buf).await?;
    Ok(())
}
