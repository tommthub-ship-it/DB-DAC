/// MySQL 프로토콜 프록시
/// 클라이언트 → 간지-DAC → 실제 MySQL
use std::sync::Arc;

use anyhow::Result;
use audit::types::{AuditEvent, EventType};
use bytes::{BufMut, BytesMut};
use policy::types::{AccessRequest, DbType};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

use crate::ProxyContext;

fn upstream_addr() -> String {
    std::env::var("MYSQL_UPSTREAM").unwrap_or_else(|_| "127.0.0.1:3306".to_string())
}

pub async fn run_proxy(ctx: Arc<ProxyContext>, listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    info!("MySQL 프록시 수신 대기: {}", listen_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        let ctx = ctx.clone();
        let client_ip = client_addr.ip().to_string();

        tokio::spawn(async move {
            info!(client_ip = %client_ip, "새 MySQL 연결");
            if let Err(e) = handle_connection(ctx, client_stream, client_ip).await {
                error!("MySQL 연결 처리 오류: {}", e);
            }
        });
    }
}

async fn handle_connection(
    ctx: Arc<ProxyContext>,
    mut client: TcpStream,
    client_ip: String,
) -> Result<()> {
    let upstream_addr = upstream_addr();

    // 1. 업스트림에 먼저 연결 (MySQL은 서버가 먼저 Handshake 패킷 전송)
    let mut upstream = match TcpStream::connect(&upstream_addr).await {
        Ok(s) => s,
        Err(e) => {
            error!("MySQL 업스트림 연결 실패 {}: {}", upstream_addr, e);
            return Err(e.into());
        }
    };

    // 2. 서버 Handshake 패킷 읽기 (Initial Handshake)
    let handshake = read_mysql_packet(&mut upstream).await?;

    // 3. 클라이언트에 Handshake 전달
    write_mysql_packet(&mut client, &handshake).await?;

    // 4. 클라이언트 HandshakeResponse 읽기 (로그인 정보 포함)
    let client_response = read_mysql_packet(&mut client).await?;
    let (db_user, db_name) = parse_handshake_response(&client_response);

    // 5. 정책 평가
    let req = AccessRequest::new(&client_ip, &db_user, DbType::MySQL, &db_name);
    let result = ctx.policy.evaluate(&req);

    // 6. 감사 로그
    let event_type = if result.allowed {
        EventType::ConnectionAllowed
    } else {
        EventType::ConnectionDenied
    };
    let mut audit_event = AuditEvent::new(
        event_type,
        &client_ip,
        &db_user,
        "mysql",
        &db_name,
        result.allowed,
        &result.reason,
    );
    audit_event.matched_rule = result.matched_rule.clone();
    ctx.audit.log(audit_event).await;

    // 7. 차단 처리
    if !result.allowed {
        warn!(
            client_ip = %client_ip,
            db_user = %db_user,
            "MySQL 연결 차단: {}",
            result.reason
        );
        send_mysql_error(&mut client, 1045, "28000", &result.reason).await?;
        return Ok(());
    }

    // 8. HandshakeResponse를 업스트림으로 전달
    write_mysql_packet(&mut upstream, &client_response).await?;

    info!(
        client_ip = %client_ip,
        db_user = %db_user,
        db_name = %db_name,
        "MySQL 연결 터널링 시작"
    );

    // 9. 양방향 터널 (쿼리 인터셉트 포함)
    tunnel_with_inspection(ctx, client, upstream, client_ip, db_user, db_name).await?;

    Ok(())
}

/// 양방향 터널 (COM_QUERY 인터셉트)
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

                    // MySQL 패킷: [length 3바이트][seq 1바이트][payload]
                    // COM_QUERY = 0x03
                    if n > 4 && data[4] == 0x03 {
                        if let Ok(query) = std::str::from_utf8(&data[5..n]) {
                            let mut req = AccessRequest::new(
                                &client_ip_c,
                                &db_user_c,
                                DbType::MySQL,
                                &db_name_c,
                            );
                            req.query = Some(query.trim_end_matches('\0').to_string());
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
                                "mysql",
                                &db_name_c,
                                result.allowed,
                                &result.reason,
                            );
                            event.query = req.query.clone();
                            event.matched_rule = result.matched_rule.clone();
                            ctx_c2u.audit.log(event).await;

                            if !result.allowed {
                                warn!(query = ?req.query, "MySQL 쿼리 차단");
                                // 차단 — 업스트림으로 전달하지 않음
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

    // 업스트림 → 클라이언트
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

// ── MySQL 패킷 헬퍼 ──────────────────────────────────────

async fn read_mysql_packet(stream: &mut TcpStream) -> Result<Vec<u8>> {
    // 헤더 4바이트 읽기 (length 3 + seq 1)
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    let length = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await?;

    let mut packet = header.to_vec();
    packet.extend_from_slice(&payload);
    Ok(packet)
}

async fn write_mysql_packet(stream: &mut TcpStream, packet: &[u8]) -> Result<()> {
    stream.write_all(packet).await?;
    Ok(())
}

/// HandshakeResponse에서 user, database 파싱 (MySQL 4.1+ 프로토콜)
fn parse_handshake_response(packet: &[u8]) -> (String, String) {
    // 패킷 구조: [4 header][4 capabilities][4 max_packet][1 charset][23 reserved][user\0][auth][db\0]
    if packet.len() < 36 {
        return ("unknown".to_string(), "unknown".to_string());
    }

    let payload = &packet[4..]; // 헤더 제거
    let mut pos = 4 + 4 + 1 + 23; // capabilities(4) + max_packet(4) + charset(1) + reserved(23)

    if pos >= payload.len() {
        return ("unknown".to_string(), "unknown".to_string());
    }

    // user 읽기 (null-terminated)
    let user_end = payload[pos..].iter().position(|&b| b == 0).unwrap_or(0);
    let user = String::from_utf8_lossy(&payload[pos..pos + user_end]).to_string();
    pos += user_end + 1;

    // auth 길이 스킵
    if pos >= payload.len() {
        return (user, "unknown".to_string());
    }
    let auth_len = payload[pos] as usize;
    pos += 1 + auth_len;

    // database 읽기 (null-terminated)
    if pos >= payload.len() {
        return (user, String::new());
    }
    let db_end = payload[pos..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(payload.len() - pos);
    let db = String::from_utf8_lossy(&payload[pos..pos + db_end]).to_string();

    (user, db)
}

/// MySQL 에러 패킷 전송
async fn send_mysql_error(
    stream: &mut TcpStream,
    error_code: u16,
    sql_state: &str,
    message: &str,
) -> Result<()> {
    let mut payload = BytesMut::new();
    payload.put_u8(0xff); // ERR packet marker
    payload.put_u16_le(error_code);
    payload.put_u8(b'#');
    payload.put_slice(sql_state.as_bytes());
    payload.put_slice(message.as_bytes());

    let len = payload.len() as u32;
    let mut packet = BytesMut::new();
    packet.put_u8((len & 0xff) as u8);
    packet.put_u8(((len >> 8) & 0xff) as u8);
    packet.put_u8(((len >> 16) & 0xff) as u8);
    packet.put_u8(0x00); // sequence id
    packet.put_slice(&payload);

    stream.write_all(&packet).await?;
    Ok(())
}
