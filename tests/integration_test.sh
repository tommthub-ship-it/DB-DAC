#!/bin/bash
# 간지-DAC 통합 테스트
# 실행: bash tests/integration_test.sh

BASE_URL="http://localhost:8080"
API_KEY="change-me-in-production"
PASS=0
FAIL=0

check() {
  local name=$1
  local expected=$2
  local actual=$3
  if echo "$actual" | grep -q "$expected"; then
    echo "✅ PASS: $name"
    PASS=$((PASS+1))
  else
    echo "❌ FAIL: $name (expected '$expected', got '$actual')"
    FAIL=$((FAIL+1))
  fi
}

echo "🔐 간지-DAC 통합 테스트 시작"
echo "================================"

# 1. 헬스체크
R=$(curl -s $BASE_URL/health)
check "헬스체크" '"status":"ok"' "$R"

# 2. 인증 없이 API 접근 (401 예상)
R=$(curl -s -o /dev/null -w "%{http_code}" $BASE_URL/api/rules)
check "인증 없이 접근 차단" "401" "$R"

# 3. 규칙 목록 조회
R=$(curl -s -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules)
check "규칙 목록 조회" '"rules"' "$R"

# 4. 규칙 생성
R=$(curl -s -X POST -H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" \
  -d '{"id":"test-rule","name":"테스트 규칙","priority":999,"action":"allow","enabled":true,"conditions":[{"type":"ip_range","cidr":"192.168.99.0/24"}]}' \
  $BASE_URL/api/rules)
check "규칙 생성" '"id":"test-rule"' "$R"

# 5. 규칙 조회
R=$(curl -s -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules/test-rule)
check "규칙 단건 조회" '"test-rule"' "$R"

# 6. 규칙 토글
R=$(curl -s -X POST -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules/test-rule/toggle)
check "규칙 토글" '"enabled"' "$R"

# 시뮬레이션 테스트를 위한 내부망 허용 규칙 사전 등록
curl -s -X POST -H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" \
  -d '{"id":"allow-internal-network","name":"내부 네트워크 허용","priority":50,"action":"allow","enabled":true,"conditions":[{"type":"ip_range","cidr":"10.0.0.0/8"}]}' \
  $BASE_URL/api/rules > /dev/null

# 7. 시뮬레이션 - 허용 (내부 IP)
R=$(curl -s -X POST -H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" \
  -d '{"client_ip":"10.0.0.1","db_user":"app","db_type":"postgresql","target_db":"mydb"}' \
  $BASE_URL/api/audit/simulate)
check "시뮬레이션 - 내부IP 허용" '"allowed":true' "$R"

# 8. 시뮬레이션 - 차단 (위험 쿼리)
R=$(curl -s -X POST -H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" \
  -d '{"client_ip":"10.0.0.1","db_user":"app","db_type":"postgresql","target_db":"mydb","query":"DROP TABLE users"}' \
  $BASE_URL/api/audit/simulate)
check "시뮬레이션 - DROP 쿼리 차단" '"allowed":false' "$R"

# 9. 규칙 삭제
R=$(curl -s -X DELETE -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules/test-rule)
check "규칙 삭제" '"test-rule"' "$R"

# 10. 규칙 내보내기
R=$(curl -s -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules/export)
check "규칙 내보내기" '"rules"' "$R"

# 테스트용 내부망 규칙 정리
curl -s -X DELETE -H "Authorization: Bearer $API_KEY" $BASE_URL/api/rules/allow-internal-network > /dev/null

echo "================================"
echo "결과: ✅ $PASS 통과 / ❌ $FAIL 실패"
[ $FAIL -eq 0 ] && exit 0 || exit 1
