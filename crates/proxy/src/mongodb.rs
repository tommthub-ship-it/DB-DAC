/// MongoDB Wire Protocol 프록시
/// 클라이언트 → 간지-DAC → 실제 MongoDB
///
/// 지원 메시지:
///   OP_MSG (opCode 2013) - MongoDB 3.6+ 표준 메시지 포맷
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

const OP_MSG: i32 = 2013;

fn upstream_addr() -> String {
    std::env::var("MONGO_UPSTREAM").unwrap_or_else(|_| "127.0.0.1:27017".to_string())
}

pub async fn run_proxy(ctx: Arc<ProxyContext>, listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    info!("MongoDB 프록시 수신 대기: {}", listen_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        let ctx = ctx.clone();
        let client_ip = client_addr.ip().to_string();

        tokio::spawn(async move {
            info!(client_ip = %client_ip, "새 MongoDB 연결");
            if let Err(e) = handle_connection(ctx, client_stream, client_ip).await {
                error!("MongoDB 연결 처리 오류: {}", e);
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
            error!("MongoDB 업스트림 연결 실패 {}: {}", upstream_addr, e);
            return Err(e.into());
        }
    };

    // 연결 허용 감사 로그
    let req = AccessRequest::new(&client_ip, "unknown", DbType::MongoDB, "mongodb");
    let result = ctx.policy.evaluate(&req);

    let mut conn_event = AuditEvent::new(
        if result.allowed {
            EventType::ConnectionAllowed
        } else {
            EventType::ConnectionDenied
        },
        &client_ip,
        "unknown",
        "mongodb",
        "mongodb",
        result.allowed,
        &result.reason,
    );
    conn_event.matched_rule = result.matched_rule.clone();
    ctx.audit.log(conn_event).await;

    if !result.allowed {
        warn!(client_ip = %client_ip, "MongoDB 연결 차단: {}", result.reason);
        // MongoDB 연결 레벨 에러: 소켓 닫기
        return Ok(());
    }

    info!(client_ip = %client_ip, "MongoDB 연결 터널링 시작");
    tunnel_with_inspection(ctx, client, upstream, client_ip).await
}

/// 양방향 터널 (OP_MSG 인터셉트)
async fn tunnel_with_inspection(
    ctx: Arc<ProxyContext>,
    client: TcpStream,
    upstream: TcpStream,
    client_ip: String,
) -> Result<()> {
    let (mut client_read, client_write_half) = client.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    // client_write를 c2u(에러 응답)와 u2c(업스트림 응답 포워딩) 양쪽에서 사용
    let client_write = Arc::new(AsyncMutex::new(client_write_half));
    let client_write_c = client_write.clone();
    let client_write_u = client_write.clone();

    let ctx_c2u = ctx.clone();
    let client_ip_c = client_ip.clone();

    // 클라이언트 → 업스트림 (쿼리 인터셉트)
    let c2u = tokio::spawn(async move {
        loop {
            let msg = match read_mongo_message(&mut client_read).await {
                Ok(m) => m,
                Err(_) => break,
            };

            // 메시지 헤더 파싱: [length:i32][requestId:i32][responseTo:i32][opCode:i32]
            if msg.len() < 16 {
                if upstream_write.write_all(&msg).await.is_err() {
                    break;
                }
                continue;
            }

            let op_code = i32::from_le_bytes([msg[12], msg[13], msg[14], msg[15]]);
            let request_id = i32::from_le_bytes([msg[4], msg[5], msg[6], msg[7]]);

            if op_code == OP_MSG {
                let payload = &msg[16..]; // 헤더(16바이트) 이후
                let command = extract_mongo_command(payload).unwrap_or_default();

                if !command.is_empty() {
                    let mut req =
                        AccessRequest::new(&client_ip_c, "unknown", DbType::MongoDB, "mongodb");
                    req.query = Some(command.clone());
                    let result = ctx_c2u.policy.evaluate(&req);

                    let event_type = if result.allowed {
                        EventType::QueryExecuted
                    } else {
                        EventType::QueryBlocked
                    };
                    let mut event = AuditEvent::new(
                        event_type,
                        &client_ip_c,
                        "unknown",
                        "mongodb",
                        "mongodb",
                        result.allowed,
                        &result.reason,
                    );
                    event.query = Some(command.clone());
                    event.matched_rule = result.matched_rule.clone();
                    ctx_c2u.audit.log(event).await;

                    if !result.allowed {
                        warn!(command = %command, client_ip = %client_ip_c, "MongoDB 커맨드 차단");
                        let err_msg = build_opmsg_error(
                            request_id,
                            &format!("Access denied by ganji-DAC: {}", result.reason),
                        );
                        if client_write_c.lock().await.write_all(&err_msg).await.is_err() {
                            break;
                        }
                        continue;
                    }
                }
            }

            if upstream_write.write_all(&msg).await.is_err() {
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

// ── MongoDB Wire Protocol 헬퍼 ───────────────────────────

/// 전체 MongoDB 메시지를 읽는다 (4바이트 길이 필드 포함)
async fn read_mongo_message<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: AsyncReadExt + Unpin,
{
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;

    let total_len = i32::from_le_bytes(len_bytes) as usize;
    if total_len < 16 || total_len > 48 * 1024 * 1024 {
        return Err(anyhow::anyhow!("잘못된 MongoDB 메시지 길이: {}", total_len));
    }

    let mut msg = vec![0u8; total_len];
    msg[0..4].copy_from_slice(&len_bytes);
    reader.read_exact(&mut msg[4..]).await?;
    Ok(msg)
}

/// OP_MSG 페이로드에서 MongoDB 커맨드 이름 추출
///
/// OP_MSG 구조:
///   [flagBits:u32][sections...]
///   Section kind 0: [kind=0:u8][BSON document]
///   BSON document 첫 번째 키 = 커맨드 이름
fn extract_mongo_command(payload: &[u8]) -> Option<String> {
    if payload.len() < 5 {
        return None;
    }
    // flagBits 4바이트 스킵
    let mut pos = 4usize;

    let kind = payload[pos];
    pos += 1;

    if kind != 0 {
        // kind 1 (Document Sequence)은 커맨드 추출 불필요
        return None;
    }

    // BSON document: [totalSize:i32][elements][0x00]
    if pos + 4 > payload.len() {
        return None;
    }
    // document 크기 필드 스킵
    pos += 4;

    // 첫 번째 element type
    if pos >= payload.len() {
        return None;
    }
    let elem_type = payload[pos];
    pos += 1;

    if elem_type == 0x00 {
        return None; // 빈 document
    }

    // element 키(CString) = 커맨드 이름
    let key_start = pos;
    while pos < payload.len() && payload[pos] != 0 {
        pos += 1;
    }
    if pos >= payload.len() {
        return None;
    }

    let key = std::str::from_utf8(&payload[key_start..pos]).ok()?;
    Some(key.to_lowercase())
}

/// MongoDB OP_MSG 에러 응답 생성
///
/// { "ok": 0, "errmsg": "...", "code": 13, "$db": "admin" }
fn build_opmsg_error(response_to: i32, message: &str) -> Vec<u8> {
    // BSON document 구성
    let mut bson_body = BytesMut::new();

    // ok: 0 (int32, type 0x10)
    bson_body.put_u8(0x10);
    bson_body.put_slice(b"ok\0");
    bson_body.put_i32_le(0);

    // errmsg: string (type 0x02)
    bson_body.put_u8(0x02);
    bson_body.put_slice(b"errmsg\0");
    let msg_bytes = message.as_bytes();
    bson_body.put_i32_le((msg_bytes.len() + 1) as i32);
    bson_body.put_slice(msg_bytes);
    bson_body.put_u8(0x00);

    // code: 13 (Unauthorized, int32)
    bson_body.put_u8(0x10);
    bson_body.put_slice(b"code\0");
    bson_body.put_i32_le(13);

    // document 종료
    bson_body.put_u8(0x00);

    // document 전체 크기(4바이트 size 필드 포함)
    let doc_size = (bson_body.len() + 4) as i32;
    let mut doc = BytesMut::new();
    doc.put_i32_le(doc_size);
    doc.put_slice(&bson_body);

    // OP_MSG 페이로드: [flagBits:u32][kind=0:u8][doc]
    let mut opmsg = BytesMut::new();
    opmsg.put_u32_le(0); // flagBits
    opmsg.put_u8(0x00);  // section kind 0
    opmsg.put_slice(&doc);

    // Wire 메시지 헤더: [length:i32][requestId:i32][responseTo:i32][opCode:i32]
    let total_len = (16 + opmsg.len()) as i32;
    let mut wire = BytesMut::new();
    wire.put_i32_le(total_len);
    wire.put_i32_le(0);           // requestId
    wire.put_i32_le(response_to); // responseTo
    wire.put_i32_le(OP_MSG);      // opCode
    wire.put_slice(&opmsg);

    wire.to_vec()
}
