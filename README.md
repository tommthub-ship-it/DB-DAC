# 🔐 간지-DAC

> 대한민국 규제(개인정보보호법, ISMS-P) 준수 AWS 최적화 DB 접근제어 솔루션

## 아키텍처

```
[클라이언트] → [간지-DAC Proxy] → [실제 DB]
                     │
              ┌──────┴──────┐
         [Policy Engine] [Audit Logger]
              │                  │
         [AWS IAM /         [CloudWatch /
         Secrets Manager]    S3 / File]
```

## 컴포넌트

| 크레이트 | 역할 |
|---|---|
| `proxy` | DB 프로토콜 프록시 (PostgreSQL 시작, 확장 예정) |
| `policy` | 접근 정책 엔진 (Deny-by-default) |
| `audit` | 감사 로그 (파일 / CloudWatch) |
| `api` | 관리 REST API (예정) |

## 규제 대응

- **개인정보보호법 §29** — 기술적 보호조치 (접근통제, 감사로그)
- **ISMS-P** — 최소권한, 계정분리, 접근이력
- **위험 쿼리 차단** — DROP / TRUNCATE / 조건없는 DELETE·UPDATE
- **업무시간 외 접근 경고** — 평일 18시 이후 Alert

## 기본 정책 (Deny-by-default)

명시적 허용 규칙 없으면 전부 차단.

## 빌드

```bash
cargo build --release
```

## 환경변수

| 변수 | 기본값 | 설명 |
|---|---|---|
| `PG_UPSTREAM` | `127.0.0.1:5432` | 실제 PostgreSQL 주소 |
| `RUST_LOG` | `info` | 로그 레벨 |
