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

---

## 🐳 Docker Compose (개발 환경)

```bash
# 전체 스택 실행 (proxy + api + console + redis + postgres + log-viewer)
docker compose --profile dev up -d

# 헬스체크
curl http://localhost:8080/health

# 관리 콘솔
open http://localhost:3000

# 로그 뷰어 (Dozzle)
open http://localhost:9999
```

---

## ☸️ Kubernetes / Helm 배포

### 사전 요구사항

- Kubernetes 1.22+
- Helm 3.0+

### 설치

```bash
# 기본 설치
helm install ganji-dac ./helm/ganji-dac \
  --namespace ganji-dac \
  --create-namespace

# 운영 환경 (RDS 연동)
helm install ganji-dac ./helm/ganji-dac \
  --namespace ganji-dac \
  --create-namespace \
  --set proxy.upstream.postgres="your-rds.cluster.ap-northeast-2.rds.amazonaws.com:5432" \
  --set proxy.aws.region="ap-northeast-2" \
  --set proxy.aws.secretId="your-secrets-manager-id" \
  --set api.secret.apiKey="your-strong-api-key" \
  --set redis.password="your-strong-password"

# Ingress 활성화
helm install ganji-dac ./helm/ganji-dac \
  --namespace ganji-dac \
  --create-namespace \
  --set ingress.enabled=true \
  --set ingress.console.host="ganji-dac.yourdomain.com" \
  --set ingress.api.host="api.ganji-dac.yourdomain.com"
```

### 업그레이드

```bash
helm upgrade ganji-dac ./helm/ganji-dac \
  --namespace ganji-dac \
  --reuse-values
```

### 제거

```bash
helm uninstall ganji-dac --namespace ganji-dac
```

### Helm values 주요 항목

| 경로 | 기본값 | 설명 |
|---|---|---|
| `proxy.replicas` | `2` | 프록시 레플리카 수 |
| `proxy.upstream.postgres` | `""` | PostgreSQL RDS 엔드포인트 |
| `proxy.aws.region` | `ap-northeast-2` | AWS 리전 |
| `proxy.aws.secretId` | `""` | Secrets Manager 시크릿 ID |
| `api.secret.apiKey` | `change-me-in-production` | API 인증 키 |
| `redis.password` | `ganji-dac-secret` | Redis 비밀번호 |
| `audit.cloudwatch.enabled` | `false` | CloudWatch 감사 로그 활성화 |
| `ingress.enabled` | `false` | Ingress 활성화 |

### 차트 검증

```bash
helm lint ./helm/ganji-dac
```

---

## 🧪 통합 테스트

```bash
# Docker 스택이 실행 중인 상태에서
bash tests/integration_test.sh
```

테스트 항목:
1. 헬스체크
2. 인증 없이 접근 차단 (401)
3. 규칙 목록 조회
4. 규칙 생성
5. 규칙 단건 조회
6. 규칙 토글
7. 시뮬레이션 - 내부 IP 허용
8. 시뮬레이션 - DROP 쿼리 차단
9. 규칙 삭제
10. 규칙 내보내기
