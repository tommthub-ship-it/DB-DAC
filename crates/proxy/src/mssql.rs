/// MSSQL TDS 프록시
/// 클라이언트 → 간지-DAC → 실제 MSSQL
///
/// TDS(Tabular Data Stream) 프로토콜:
///   패킷 헤더: [type:u8][status:u8][length:u16 BE][spid:u16][packetId:u8][window:u8]
///
///   PacketType:
///     0x01 = SQL Batch
///     0x02 = Pre-TDS7 Login
///     0x10 = TDS7 Login (LOGIN7)
///     0x12 = Pre-Login
///
/// SQL Batch 페이로드는 UTF-16LE 인코딩
use std::sync::Arc;

use anyhow::Result;
use audit::types::{AuditEvent, EventType};
use bytes::{BufMut, BytesMut};
use policy::types::{AccessRequest, DbType};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};
use tracing::{error, info, warn};

use crate::ProxyContext;

const TDS_PACKET_SQL_BATCH: u8 = 0x01;
const TDS_PACKET_LOGIN7: u8 = 0x10;
const TDS_PACKET_PRELOGIN: u8 = 0x12;
const TDS_HEADER_LEN: usize = 8;

fn upstream_addr() -> String {
    std::env::var("MSSQL_UPSTREAM").unwrap_or_else(|_| "127.0.0.1:1433".to_string())
}

pub async fn run_proxy(ctx: Arc<ProxyContext>, listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    info!("MSSQL 프록시 수신 대기: {}", listen_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        let ctx = ctx.clone();
        let client_ip = client_addr.ip().to_string();

        tokio::spawn(async move {
            info!(client_ip = %client_ip, "새 MSSQL 연결");
            if let Err(e) = handle_connection(ctx, client_stream, client_ip).await {
                error!("MSSQL 연결 처리 오류: {}", e);
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

    // 1. 클라이언트 PRELOGIN 읽기 (TDS에서 클라이언트가 먼저 전송)
    let prelogin = match read_tds_message(&mut client).await {
        Ok(p) => p,
        Err(e) => {
            error!("MSSQL PRELOGIN 읽기 실패: {}", e);
            return Err(e);
        }
    };

    if prelogin.is_empty() || prelogin[0] != TDS_PACKET_PRELOGIN {
        warn!(client_ip = %client_ip, "MSSQL 첫 패킷이 PRELOGIN이 아님: type=0x{:02x}", prelogin.first().unwrap_or(&0));
    }

    // 2. 업스트림 연결
    let mut upstream = match TcpStream::connect(&upstream_addr).await {
        Ok(s) => s,
        Err(e) => {
            error!("MSSQL 업스트림 연결 실패 {}: {}", upstream_addr, e);
            return Err(e.into());
        }
    };

    // 3. PRELOGIN 전달 → 업스트림 응답 → 클라이언트 반환
    upstream.write_all(&prelogin).await?;
    let prelogin_resp = read_tds_message(&mut upstream).await?;
    client.write_all(&prelogin_resp).await?;

    // 4. LOGIN7 읽기 (또는 SSL 핸드셰이크 후 LOGIN7)
    //    SSL 협상이 시작되면(PRELOGIN 응답 ENCRYPT_ON) 이후 패킷은 암호화됨
    //    → 암호화 여부 확인 후 투명 터널로 폴백
    if is_encrypted_prelogin(&prelogin_resp) {
        // TLS 협상 중 → 투명 터널
        warn!(client_ip = %client_ip, "MSSQL TLS 암호화 감지, 투명 터널 모드로 전환");

        // 연결 허용 감사 로그 (쿼리 인스펙션 불가)
        let req = AccessRequest::new(&client_ip, "unknown", DbType::MSSQL, "mssql");
        let result = ctx.policy.evaluate(&req);
        let mut event = AuditEvent::new(
            if result.allowed {
                EventType::ConnectionAllowed
            } else {
                EventType::ConnectionDenied
            },
            &client_ip, "unknown", "mssql", "mssql",
            result.allowed, &result.reason,
        );
        event.matched_rule = result.matched_rule.clone();
        ctx.audit.log(event).await;

        return transparent_tunnel(client, upstream).await;
    }

    // 5. LOGIN7 읽기
    let login7_pkt = read_tds_message(&mut client).await?;
    let (db_user, db_name) = if login7_pkt.first() == Some(&TDS_PACKET_LOGIN7) {
        parse_login7(&login7_pkt)
    } else {
        ("unknown".to_string(), "mssql".to_string())
    };

    // 6. 연결 레벨 정책 평가
    let req = AccessRequest::new(&client_ip, &db_user, DbType::MSSQL, &db_name);
    let result = ctx.policy.evaluate(&req);

    let event_type = if result.allowed {
        EventType::ConnectionAllowed
    } else {
        EventType::ConnectionDenied
    };
    let mut conn_event = AuditEvent::new(
        event_type, &client_ip, &db_user, "mssql", &db_name,
        result.allowed, &result.reason,
    );
    conn_event.matched_rule = result.matched_rule.clone();
    ctx.audit.log(conn_event).await;

    if !result.allowed {
        warn!(
            client_ip = %client_ip,
            db_user = %db_user,
            "MSSQL 연결 차단: {}",
            result.reason
        );
        send_tds_error(&mut client, &result.reason).await?;
        return Ok(());
    }

    // 7. LOGIN7 업스트림 전달 + 응답 반환
    upstream.write_all(&login7_pkt).await?;

    // 로그인 응답은 여러 패킷일 수 있으므로 DONE 토큰 포함 패킷까지 포워딩
    forward_tds_response(&mut upstream, &mut client).await?;

    info!(
        client_ip = %client_ip,
        db_user = %db_user,
        db_name = %db_name,
        "MSSQL 연결 터널링 시작"
    );

    // 8. 양방향 터널 (SQL Batch 인터셉트)
    tunnel_with_inspection(ctx, client, upstream, client_ip, db_user, db_name).await
}

/// 양방향 터널 (SQL Batch 인터셉트)
async fn tunnel_with_inspection(
    ctx: Arc<ProxyContext>,
    client: TcpStream,
    upstream: TcpStream,
    client_ip: String,
    db_user: String,
    db_name: String,
) -> Result<()> {
    let (mut client_read, client_write_half) = client.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    // c2u(에러 응답)와 u2c(업스트림 응답 포워딩) 양쪽에서 사용
    let client_write = Arc::new(AsyncMutex::new(client_write_half));
    let client_write_c = client_write.clone();
    let client_write_u = client_write.clone();

    let ctx_c2u = ctx.clone();
    let client_ip_c = client_ip.clone();
    let db_user_c = db_user.clone();
    let db_name_c = db_name.clone();

    // 클라이언트 → 업스트림 (SQL Batch 인터셉트)
    let c2u = tokio::spawn(async move {
        loop {
            let pkt = match read_tds_message(&mut client_read).await {
                Ok(p) => p,
                Err(_) => break,
            };

            if pkt.first() == Some(&TDS_PACKET_SQL_BATCH) {
                // 페이로드(8바이트 헤더 이후) → UTF-16LE 디코딩
                let query = extract_sql_batch_query(&pkt);

                let mut req = AccessRequest::new(
                    &client_ip_c, &db_user_c, DbType::MSSQL, &db_name_c,
                );
                req.query = Some(query.clone());
                let result = ctx_c2u.policy.evaluate(&req);

                let event_type = if result.allowed {
                    EventType::QueryExecuted
                } else {
                    EventType::QueryBlocked
                };
                let mut event = AuditEvent::new(
                    event_type, &client_ip_c, &db_user_c, "mssql", &db_name_c,
                    result.allowed, &result.reason,
                );
                event.query = Some(query.clone());
                event.matched_rule = result.matched_rule.clone();
                ctx_c2u.audit.log(event).await;

                if !result.allowed {
                    warn!(
                        client_ip = %client_ip_c,
                        db_user = %db_user_c,
                        "MSSQL 쿼리 차단: {}",
                        result.reason
                    );
                    let err_pkt = build_tds_error_packet(&result.reason);
                    if client_write_c.lock().await.write_all(&err_pkt).await.is_err() {
                        break;
                    }
                    continue;
                }
            }

            if upstream_write.write_all(&pkt).await.is_err() {
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

// ── TDS 프로토콜 헬퍼 ────────────────────────────────────

/// TDS 패킷/메시지 읽기
///
/// TDS 헤더: [type:1][status:1][length:2 BE][spid:2][packetId:1][window:1]
/// length에는 헤더(8바이트)가 포함됨
/// status & 0x01 = Last in message (EOM)
///
/// 여러 패킷으로 나뉜 메시지는 하나로 합쳐서 반환
async fn read_tds_message<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: AsyncReadExt + Unpin,
{
    let mut full_message = Vec::new();

    loop {
        let mut header = [0u8; TDS_HEADER_LEN];
        reader.read_exact(&mut header).await?;

        let pkt_len = u16::from_be_bytes([header[2], header[3]]) as usize;
        if pkt_len < TDS_HEADER_LEN || pkt_len > 65536 {
            return Err(anyhow::anyhow!("잘못된 TDS 패킷 길이: {}", pkt_len));
        }

        let payload_len = pkt_len - TDS_HEADER_LEN;
        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload).await?;

        // 첫 패킷이면 헤더 포함, 이후 패킷은 페이로드만 병합
        if full_message.is_empty() {
            full_message.extend_from_slice(&header);
        }
        full_message.extend_from_slice(&payload);

        let status = header[1];
        // status & 0x01 = EOM (End of Message)
        if status & 0x01 != 0 {
            break;
        }
    }

    Ok(full_message)
}

/// PRELOGIN 응답에 암호화(ENCRYPT_ON=1 또는 ENCRYPT_REQ=3) 플래그 확인
fn is_encrypted_prelogin(prelogin_resp: &[u8]) -> bool {
    if prelogin_resp.len() <= TDS_HEADER_LEN {
        return false;
    }
    let payload = &prelogin_resp[TDS_HEADER_LEN..];

    // PRELOGIN 토큰 파싱: [type:1][offset:2 BE][length:2 BE] ... 0xFF
    let mut pos = 0usize;
    while pos + 5 <= payload.len() {
        let token_type = payload[pos];
        if token_type == 0xFF {
            break; // TERMINATOR
        }
        let offset = u16::from_be_bytes([payload[pos + 1], payload[pos + 2]]) as usize;
        let length = u16::from_be_bytes([payload[pos + 3], payload[pos + 4]]) as usize;
        pos += 5;

        // ENCRYPTION token = 0x01
        if token_type == 0x01 && length >= 1 && offset < payload.len() {
            let enc_value = payload[offset];
            // 0x01 = ENCRYPT_ON, 0x03 = ENCRYPT_REQ
            return enc_value == 0x01 || enc_value == 0x03;
        }
    }
    false
}

/// LOGIN7 패킷에서 username과 database 이름 추출
///
/// LOGIN7 페이로드 구조 (TDS 7.4):
///   [4 TotalLength][4 TDSVersion][4 PacketSize][4 ClientProgVer][4 ClientPID]
///   [4 ConnectionID][1 OptionFlags1][1 OptionFlags2][1 TypeFlags][1 OptionFlags3]
///   [4 ClientTimeZone][4 ClientLCID]
///   [2 ibHostName][2 cchHostName]
///   [2 ibUserName][2 cchUserName]
///   [2 ibPassword][2 cchPassword]
///   ...
///   [2 ibDatabase][2 cchDatabase]
///   ...
///   [가변 데이터 영역 - UTF-16LE]
fn parse_login7(packet: &[u8]) -> (String, String) {
    if packet.len() < TDS_HEADER_LEN + 90 {
        return ("unknown".to_string(), "mssql".to_string());
    }

    let payload = &packet[TDS_HEADER_LEN..]; // TDS 헤더(8) 제거

    // ibUserName: offset 40, cchUserName: offset 42
    let ib_username = u16::from_le_bytes([payload[40], payload[41]]) as usize;
    let cch_username = u16::from_le_bytes([payload[42], payload[43]]) as usize;

    // ibDatabase: offset 68, cchDatabase: offset 70
    let ib_database = u16::from_le_bytes([payload[68], payload[69]]) as usize;
    let cch_database = u16::from_le_bytes([payload[70], payload[71]]) as usize;

    let username = read_utf16le(payload, ib_username, cch_username);
    let database = read_utf16le(payload, ib_database, cch_database);

    let username = if username.is_empty() {
        "unknown".to_string()
    } else {
        username
    };
    let database = if database.is_empty() {
        "mssql".to_string()
    } else {
        database
    };

    (username, database)
}

/// 페이로드에서 UTF-16LE 문자열 읽기
fn read_utf16le(payload: &[u8], offset: usize, char_count: usize) -> String {
    if char_count == 0 || offset + char_count * 2 > payload.len() {
        return String::new();
    }
    let bytes = &payload[offset..offset + char_count * 2];
    let u16_chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16_chars).to_string()
}

/// SQL Batch 패킷에서 UTF-16LE 쿼리 추출
fn extract_sql_batch_query(packet: &[u8]) -> String {
    if packet.len() <= TDS_HEADER_LEN {
        return String::new();
    }
    let payload = &packet[TDS_HEADER_LEN..];

    // TDS 7.2+: ALL_HEADERS (가변 길이) 이후가 쿼리
    // ALL_HEADERS: [TotalLength:4][헤더들...]
    // 간단한 처리: TotalLength가 있으면 스킵
    let start = if payload.len() > 4 {
        let hdr_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
        if hdr_len > 4 && hdr_len < payload.len() {
            hdr_len
        } else {
            0
        }
    } else {
        0
    };

    let query_bytes = &payload[start..];
    let u16_chars: Vec<u16> = query_bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let query = String::from_utf16_lossy(&u16_chars).to_string();
    query.trim_matches('\0').trim().to_string()
}

/// 로그인 응답 패킷(들)을 업스트림에서 클라이언트로 포워딩
///
/// 로그인 응답은 여러 토큰을 포함한 TDS 메시지
async fn forward_tds_response(upstream: &mut TcpStream, client: &mut TcpStream) -> Result<()> {
    let resp = read_tds_message(upstream).await?;
    client.write_all(&resp).await?;
    Ok(())
}

/// 단순 투명 터널 (TLS 협상 등 인스펙션 불가 시)
async fn transparent_tunnel(client: TcpStream, upstream: TcpStream) -> Result<()> {
    let (mut client_read, mut client_write) = client.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    let c2u = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            match client_read.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if upstream_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let u2c = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            match upstream_read.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if client_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let _ = tokio::join!(c2u, u2c);
    Ok(())
}

/// TDS 에러 응답 패킷 생성
///
/// ERROR 토큰(0xAA) + DONE 토큰(0xFD)
fn build_tds_error_packet(message: &str) -> Vec<u8> {
    let msg_utf16: Vec<u16> = message.encode_utf16().collect();
    let server_name_utf16: Vec<u16> = "ganji-DAC".encode_utf16().collect();

    let mut token_data = BytesMut::new();

    // ── ERROR 토큰 (0xAA) ──
    // 토큰 내용 먼저 구성
    let mut err_body = BytesMut::new();
    err_body.put_u32_le(18_456); // error number (MSSQL login-failed code)
    err_body.put_u8(1);          // state
    err_body.put_u8(14);         // severity (14 = DBPROCESS error)

    // MsgText: u16 char count + UTF-16LE
    err_body.put_u16_le(msg_utf16.len() as u16);
    for ch in &msg_utf16 {
        err_body.put_u16_le(*ch);
    }

    // ServerName: u8 char count + UTF-16LE
    err_body.put_u8(server_name_utf16.len() as u8);
    for ch in &server_name_utf16 {
        err_body.put_u16_le(*ch);
    }

    // ProcName: u8 = 0 (없음)
    err_body.put_u8(0);

    // LineNumber: u32
    err_body.put_u32_le(1);

    // 토큰 타입 + 길이(u16)
    token_data.put_u8(0xAA);
    token_data.put_u16_le(err_body.len() as u16);
    token_data.put_slice(&err_body);

    // ── DONE 토큰 (0xFD) ──
    token_data.put_u8(0xFD); // type
    token_data.put_u16_le(0x0002); // status: error occurred
    token_data.put_u16_le(0); // curCmd
    token_data.put_u64_le(0); // rowCount

    // ── TDS 패킷 헤더 ──
    let pkt_len = (TDS_HEADER_LEN + token_data.len()) as u16;
    let mut pkt = BytesMut::new();
    pkt.put_u8(0x04);                          // type: TABULAR_RESULT
    pkt.put_u8(0x01);                          // status: EOM
    pkt.put_u16(pkt_len);                      // length (big-endian)
    pkt.put_u16(0);                            // SPID
    pkt.put_u8(1);                             // packetId
    pkt.put_u8(0);                             // window
    pkt.put_slice(&token_data);

    pkt.to_vec()
}

/// 클라이언트에 TDS 에러 전송
async fn send_tds_error(client: &mut TcpStream, message: &str) -> Result<()> {
    let pkt = build_tds_error_packet(message);
    client.write_all(&pkt).await?;
    Ok(())
}
