#!/usr/bin/env bash
set -e
source "$(dirname "$0")/common.sh"

export COWEN_PROFILE="default"
setup_workspace "case_81"
trap cleanup_suite EXIT
start_mock

echo "================================================================"
echo " E2E Test Case 81: Gateway Direct OpenAPI & Multiple Upstreams  "
echo "================================================================"

export COWEN_GATEWAY_PORT=$(get_unused_port)
export COWEN_MONITOR_PORT=$(get_unused_port)
export UPSTREAM_B_PORT=$(get_unused_port)

# 1. Start a simple HTTP Service B in background using python
PYTHON_SERVICE_B="$COWEN_HOME/service_b.py"
cat <<EOF > "$PYTHON_SERVICE_B"
import sys
from http.server import SimpleHTTPRequestHandler, HTTPServer
import json

class MyHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        res = {
            "msg": "Order Service",
            "path": self.path,
            "org_id": self.headers.get('x-org-id', ''),
            "user_id": self.headers.get('x-user-id', '')
        }
        self.wfile.write(json.dumps(res).encode('utf-8'))
    def do_POST(self):
        self.do_GET()

port = int(sys.argv[1])
HTTPServer(('127.0.0.1', port), MyHandler).serve_forever()
EOF

python3 "$PYTHON_SERVICE_B" "$UPSTREAM_B_PORT" > "$COWEN_HOME/service_b.log" 2>&1 &
SERVICE_B_PID=$!
echo ">> Started Service B on port $UPSTREAM_B_PORT (PID: $SERVICE_B_PID)"

# Register private cleanup for Service B
cleanup_suite_original() {
    echo ">> Stopping Service B (PID: $SERVICE_B_PID)..."
    kill -9 "$SERVICE_B_PID" >/dev/null 2>&1 || true
    cleanup_suite
}
trap cleanup_suite_original EXIT

# Wait for Service B to be ready
for i in {1..5}; do
    if curl -s "http://127.0.0.1:${UPSTREAM_B_PORT}/" >/dev/null; then
        echo ">> Service B is ready."
        break
    fi
    sleep 1
done

# 2. Configure the gateway in default.yaml BEFORE init
# We define routes: 
# - /open-api/** -> openapi (direct/bypass) with strip_prefix /open-api
# - /order/** -> Service B with strip_prefix /order
# Default upstream_url points to Mock Server
PROFILE_YAML="$COWEN_HOME/default.yaml"
cat <<EOF > "$PROFILE_YAML"
app_key: "mock_app_key"
webhook_target: "$MOCK_URL"
app_mode: "store-app"
gateway:
  bind_address: "127.0.0.1:${COWEN_GATEWAY_PORT}"
  upstream_url: "$MOCK_URL"
  auth_routing:
    mode: "STRICT"
    bypass_rules:
      - "/v1/mock/ping"
      - "/open-api/v1/mock/ping"
    require_rules:
      - "**"
  routes:
    - path: "/open-api/**"
      upstream: "openapi"
      strip_prefix: "/open-api"
    - path: "/order/**"
      upstream: "http://127.0.0.1:${UPSTREAM_B_PORT}"
      strip_prefix: "/order"
EOF

# 3. Initialize store_app profile
"$COWEN_BIN" init \
  --app-mode store_app \
  --app-key "mock_app_key" \
  --app-secret "mock_app_secret" \
  --encrypt-key "mock_encrypt_key" \
  --openapi-url "$MOCK_URL" \
  --stream-url "$MOCK_WS"

"$COWEN_BIN" config set monitor_port "$COWEN_MONITOR_PORT"

# Start daemon (this starts the gateway)
"$COWEN_BIN" daemon start
wait_for_daemon default 10
sleep 2

GATEWAY_URL="http://127.0.0.1:${COWEN_GATEWAY_PORT}"

echo ">> Scenario 1: Default Upstream Route (No routing rules match)"
# /v1/mock/ping bypasses auth and goes to default upstream_url (Mock Server)
RES_DEFAULT=$(curl -s "$GATEWAY_URL/v1/mock/ping")
echo "Default Response: $RES_DEFAULT"
if ! echo "$RES_DEFAULT" | grep -q "status"; then
    echo "❌ Expected default upstream response containing 'status'"
    exit 1
fi

echo ">> Scenario 2: Direct OpenAPI Route (Bypass/Direct Proxy to OpenAPI)"
# 1. Establish session via code interception
COOKIE_JAR="$COWEN_HOME/cookies.txt"
curl -s -i -c "$COOKIE_JAR" "$GATEWAY_URL/home?code=code_org888" > /dev/null
sleep 2

# 2. Call /open-api/v1/mock/secure (requires session)
# Gateway should match /open-api/**, strip /open-api to /v1/mock/secure,
# call intercept_request directly inside gateway process, wash and sign,
# and direct proxy to openapi_url without local proxy daemon process running.
RES_DIRECT=$(curl -s -b "$COOKIE_JAR" "$GATEWAY_URL/open-api/v1/mock/secure")
echo "Direct OpenAPI Response: $RES_DIRECT"
if ! echo "$RES_DIRECT" | grep -q "verified"; then
    echo "❌ Expected direct OpenAPI response to contain 'verified'"
    exit 1
fi

# Assert openToken was washed and decorated properly using initial JWT token
if ! echo "$RES_DIRECT" | grep -q "fakesignature"; then
    echo "❌ Expected token_used in direct response to be the initial JWT token"
    exit 1
fi

echo ">> Scenario 2.2: Direct OpenAPI 3-Tier Recovery"
# Clean up tenant token cache and secrets to force recovery via permanent code
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_tenant_token;"
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_secret WHERE item_key LIKE 'oauth2_token_pair_%';"

# Restart daemon to clear memory caches
echo ">> Restarting Daemon to clear cache..."
PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid" 2>/dev/null || true)
if [ -n "$PID" ]; then
    kill -9 "$PID" >/dev/null 2>&1 || true
fi
sleep 1
"$COWEN_BIN" daemon start
wait_for_daemon default 10
sleep 2

# Request again. The session cookie is still valid, but since the raw token has been purged,
# the gateway must trigger 3-tier recovery (permanent code exchange) on the fly.
RES_RECOVER=$(curl -s -b "$COOKIE_JAR" "$GATEWAY_URL/open-api/v1/mock/secure")
echo "Direct OpenAPI Recovery Response: $RES_RECOVER"
if ! echo "$RES_RECOVER" | grep -q "verified"; then
    echo "❌ Expected direct recovery response to contain 'verified'"
    exit 1
fi

if ! echo "$RES_RECOVER" | grep -q "mock_at_oa2_user_permanent_code"; then
    echo "❌ Expected token_used after recovery to be a newly exchanged OAuth2 user access token"
    exit 1
fi

echo ">> Scenario 3: Multiple ISV Upstream Distribution"
# Call /order/list. It should match /order/**, strip /order to /list,
# proxy to Service B, and inject x-org-id/x-user-id headers.
RES_ORDER=$(curl -s -b "$COOKIE_JAR" "$GATEWAY_URL/order/list")
echo "Order Service Response: $RES_ORDER"
if ! echo "$RES_ORDER" | grep -q "Order Service"; then
    echo "❌ Expected Service B response containing 'Order Service'"
    exit 1
fi
if ! echo "$RES_ORDER" | grep -q "org888"; then
    echo "❌ Expected Service B response to contain injected org_id 'org888'"
    exit 1
fi
if ! echo "$RES_ORDER" | grep -q "/list"; then
    echo "❌ Expected Service B path to be stripped to '/list'"
    exit 1
fi

echo "✅ Case 81 Passed!"
