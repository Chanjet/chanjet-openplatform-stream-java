#!/usr/bin/env bash
set -e
source "$(dirname "$0")/common.sh"

export COWEN_PROFILE="default"
setup_workspace "case_80"
trap cleanup_suite EXIT
start_mock

echo "========================================="
echo " E2E Test Case 80: Identity-Aware Gateway "
echo "========================================="

export COWEN_GATEWAY_PORT=$(get_unused_port)
export COWEN_PROXY_PORT=$(get_unused_port)
export COWEN_MONITOR_PORT=$(get_unused_port)

# 1. Configure the gateway in default.yaml BEFORE init
PROFILE_YAML="$COWEN_HOME/default.yaml"
cat <<EOF > "$PROFILE_YAML"
app_key: "mock_app_key"
webhook_target: "$MOCK_URL"
app_mode: "store-app"
gateway:
  bind_address: "127.0.0.1:${COWEN_GATEWAY_PORT}"
  upstream_url: "$MOCK_URL"
  auth_sync_hook: "$MOCK_URL/mock_isv/auth_sync_hook"
  auth_routing:
    mode: "STRICT"
    bypass_rules:
      - "/v1/mock/ping"
    require_rules:
      - "**"
EOF

# 2. Initialize store_app profile with gateway enabled
"$COWEN_BIN" init \
  --app-mode store_app \
  --app-key "mock_app_key" \
  --app-secret "mock_app_secret" \
  --encrypt-key "mock_encrypt_key" \
  --openapi-url "$MOCK_URL" \
  --stream-url "$MOCK_WS"

"$COWEN_BIN" config set monitor_port "$COWEN_MONITOR_PORT"
"$COWEN_BIN" config set proxy_port "$COWEN_PROXY_PORT"

wait_for_daemon default 10

GATEWAY_URL="http://127.0.0.1:${COWEN_GATEWAY_PORT}"

echo ">> Scenario 1: CORS Fallback (401 JSON / 302 HTML)"
# JSON Request should yield 401
RES_JSON=$(curl -s -w "%{http_code}" -H "Accept: application/json" "$GATEWAY_URL/api/secure")
HTTP_CODE=${RES_JSON: -3}
BODY=${RES_JSON%???}
if [ "$HTTP_CODE" != "401" ]; then
    echo "❌ Expected 401 for JSON CORS fallback, got $HTTP_CODE"
    exit 1
fi
if ! echo "$BODY" | grep -q "login_url"; then
    echo "❌ Expected login_url in 401 response body"
    exit 1
fi

# HTML Request should yield 302
RES_HTML=$(curl -s -w "%{http_code}" -I -H "Accept: text/html" "$GATEWAY_URL/api/secure" | grep -i "^HTTP/.*302" || true)
if [ -z "$RES_HTML" ]; then
    echo "❌ Expected 302 for HTML CORS fallback"
    exit 1
fi

echo ">> Scenario 2: Code Interception & Wash & Scenario 3: Auth Sync Hook"
# Request with code
COOKIE_JAR="$COWEN_HOME/cookies.txt"
curl -s -i -c "$COOKIE_JAR" "$GATEWAY_URL/home?code=test_code_123" > "$COWEN_HOME/auth_res.txt"

# Expect 302 redirect to /home
if ! grep -q -i "HTTP/.*302" "$COWEN_HOME/auth_res.txt"; then
    echo "❌ Expected 302 after code interception"
    cat "$COWEN_HOME/auth_res.txt"
    exit 1
fi

if ! grep -q -i "Location: /home" "$COWEN_HOME/auth_res.txt"; then
    echo "❌ Expected redirect to pure URL /home"
    cat "$COWEN_HOME/auth_res.txt"
    exit 1
fi

# Expect two cookies: cowen_sess_id and isv_session
if ! grep -q "cowen_sess_id" "$COOKIE_JAR"; then
    echo "❌ Expected cowen_sess_id cookie"
    cat "$COOKIE_JAR"
    exit 1
fi
if ! grep -q "isv_session" "$COOKIE_JAR"; then
    echo "❌ Expected isv_session cookie (Auth Sync Hook)"
    cat "$COOKIE_JAR"
    exit 1
fi

echo ">> Scenario 4: Declarative Routing & Bypass"
# Request without cookie to bypass
RES_BYPASS=$(curl -s -w "%{http_code}" "$GATEWAY_URL/v1/mock/ping")
HTTP_CODE=${RES_BYPASS: -3}
if [ "$HTTP_CODE" != "200" ]; then
    echo "❌ Expected 200 for bypass route, got $HTTP_CODE"
    exit 1
fi

# Request with cookie to secure
curl -s -b "$COOKIE_JAR" "$GATEWAY_URL/v1/mock/secure" > "$COWEN_HOME/secure_res.txt"
if ! grep -q "verified" "$COWEN_HOME/secure_res.txt"; then
    echo "❌ Expected secure route to pass with valid session"
    cat "$COWEN_HOME/secure_res.txt"
    exit 1
fi

echo ">> Scenario 5: Fingerprint Binding Rejection"
# Change User-Agent to invalidate fingerprint
RES_FP=$(curl -s -w "%{http_code}" -H "Accept: application/json" -H "User-Agent: HackerAgent/1.0" -b "$COOKIE_JAR" "$GATEWAY_URL/v1/mock/secure")
HTTP_CODE=${RES_FP: -3}
if [ "$HTTP_CODE" != "401" ]; then
    echo "❌ Expected 401 due to fingerprint mismatch, got $HTTP_CODE"
    exit 1
fi

echo ">> Scenario 8: CORS Preflight"
echo "DEBUG: Running curl -v"
curl -v -X OPTIONS "$GATEWAY_URL/api/secure" || true
RES_OPTIONS=$(curl -s -i -X OPTIONS "$GATEWAY_URL/api/secure" | grep -i "Access-Control-Allow-Credentials: true" || true)
if [ -z "$RES_OPTIONS" ]; then
    echo "❌ Expected CORS preflight to pass and allow credentials"
    exit 1
fi

echo ">> Scenario 7: Egress Proxy Reuse"
# Wait until Egress is fully initialized and token stored
sleep 2

# We need to simulate ISV using proxy with x-org-id header
# In store_app mode, org_id comes from temp_code. In handle_permanent_auth_code, orgId is extracted from code or 900000000.
# Because our code was test_code_123, org_id="test_code_123". Let's check session.rs or proxy logic.
# Wait, let's use a known org id for code
curl -s -c "$COOKIE_JAR" "$GATEWAY_URL/?code=code_org999" > /dev/null
sleep 2

RES_EGRESS=$(curl -s -X POST -w "%{http_code}" -x "http://127.0.0.1:${COWEN_PROXY_PORT}" -H "x-org-id: org999" "http://127.0.0.1:${MOCK_PORT}/v1/app/data/get")
HTTP_CODE=${RES_EGRESS: -3}
if [ "$HTTP_CODE" != "200" ]; then
    echo "❌ Expected 200 via egress proxy, got $HTTP_CODE"
    exit 1
fi

restart_master_daemon() {
    PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid" 2>/dev/null || true)
    if [ -n "$PID" ]; then
        kill -9 "$PID" >/dev/null 2>&1 || true
        rm -f "$COWEN_HOME/master_daemon.pid" >/dev/null 2>&1 || true
    fi
    sleep 1
    "$COWEN_BIN" daemon start
    wait_for_daemon default 10
    sleep 2
}

echo ">> Scenario 9: 3-Tier Auth Recovery"

echo "DEBUG: sqlite3 tables:"
sqlite3 "$COWEN_HOME/cowen.db" ".tables"
sqlite3 "$COWEN_HOME/cowen.db" "SELECT * FROM cowen_token;" || true

# 1. Tier 2: Refresh Token Recovery
# Delete access_token from DB to force recovery
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_tenant_token;"

# Restart daemon to clear in-memory cache
restart_master_daemon

RES_EGRESS_RT=$(curl -s -X POST -w "%{http_code}" -x "http://127.0.0.1:${COWEN_PROXY_PORT}" -H "x-org-id: org999" "http://127.0.0.1:${MOCK_PORT}/v1/app/data/get")
HTTP_CODE_RT=${RES_EGRESS_RT: -3}
if [ "$HTTP_CODE_RT" != "200" ]; then
    echo "❌ Expected 200 via refresh token recovery, got $HTTP_CODE_RT"
    exit 1
fi

# 2. Tier 3: Permanent Auth Code Recovery
# Delete BOTH access token and refresh token (cowen_secret).
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_tenant_token;"
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_secret WHERE item_key LIKE 'oauth2_token_pair_%';"

# Restart daemon to clear in-memory cache
restart_master_daemon

RES_EGRESS_PC=$(curl -s -X POST -w "%{http_code}" -x "http://127.0.0.1:${COWEN_PROXY_PORT}" -H "x-org-id: org999" "http://127.0.0.1:${MOCK_PORT}/v1/app/data/get")
HTTP_CODE_PC=${RES_EGRESS_PC: -3}
if [ "$HTTP_CODE_PC" != "200" ]; then
    echo "❌ Expected 200 via permanent code recovery, got $HTTP_CODE_PC"
    exit 1
fi

# 3. Complete failure (No tokens at all)
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_tenant_token;"
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_secret WHERE item_key LIKE 'oauth2_token_pair_%';"
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_permanent_code;"

# Restart daemon to clear in-memory cache
restart_master_daemon

RES_EGRESS_FAIL=$(curl -s -X POST -w "%{http_code}" -x "http://127.0.0.1:${COWEN_PROXY_PORT}" -H "x-org-id: org999" "http://127.0.0.1:${MOCK_PORT}/v1/app/data/get")
HTTP_CODE_FAIL=${RES_EGRESS_FAIL: -3}
if [ "$HTTP_CODE_FAIL" != "401" ]; then
    echo "❌ Expected 401 when all recovery mechanisms fail, got $HTTP_CODE_FAIL"
    exit 1
fi

echo "✅ Case 80 Passed!"
