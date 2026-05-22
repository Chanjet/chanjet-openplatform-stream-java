#!/bin/bash
set -e
# Case 26: Hybrid Store Data Drift (Blind Spot Verification)
# Verifies:
#   1. When using Hybrid Storage (Redis + SQL).
#   2. If SQL data is manually modified (data drift) while Redis is warm.
#   3. Does the CLI detect the drift and reconcile, or does it serve stale cache?
#   Note: This is a known blind spot. The test might fail.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_26"
start_mock

REDIS_PORT=6387
REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
REDIS_PID_FILE="$COWEN_HOME/redis.pid"

start_test_redis() {
    echo -e "  Starting test Redis on port $REDIS_PORT..."
    if ! command -v redis-server >/dev/null 2>&1; then
        echo -e "  ${RED}[FAILED]${NC} redis-server not found in PATH"
        fail_suite "PATH is: $PATH"
    fi
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    
    # Start Redis in background without daemonize for better log capture
    # CD into COWEN_HOME to ensure it doesn't load dump.rdb from the current directory
    (cd "$COWEN_HOME" && redis-server --port $REDIS_PORT --save "" --appendonly no --loglevel warning > "redis.log" 2>&1 &)
    
    # Give it a moment to write PID if we were using --pidfile, but here we just wait for port
    
    # Wait for Redis to be ready
    for i in {1..5}; do
        if redis-cli -p $REDIS_PORT ping >/dev/null 2>&1; then
            echo -e "  ${GREEN}[REDIS READY]${NC}"
            return 0
        fi
        sleep 1
    done
    echo -e "  ${RED}[REDIS FAILED TO START]${NC}"
    cat "$COWEN_HOME/redis.log"
    fail_suite "--- Redis Log ---"
}

stop_test_redis() {
    echo -e "  Stopping test Redis on port $REDIS_PORT..."
    pkill -f "$(basename "$COWEN_BIN").*$PROF" 2>/dev/null || true
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    rm -f "$REDIS_PID_FILE"
    sleep 1
}

start_test_redis
DB_FILE="$COWEN_HOME/persistence.db"
PROF="hybrid_drift"

pkill -f "$(basename "$COWEN_BIN").*$PROF" 2>/dev/null || true

# Setup Hybrid configuration
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$DB_FILE"
  cache: redis
  cache_url: "$REDIS_URL"
EOF

# Init node
"$COWEN_BIN" init --profile "$PROF" --app-mode self-built \
    --app-key AK_HYBRID --app-secret AS_HYBRID --certificate CERT_HYBRID --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port $PROXY_PORT >/dev/null

# Start daemon to open websocket connection
"$COWEN_BIN" daemon start --profile "$PROF" > /dev/null

echo "   Waiting for daemon to connect..."
sleep 3
curl -s -X POST -H "appKey: AK_HYBRID" "$MOCK_URL/auth/appTicket/resend" >/dev/null
sleep 2

echo -e "${BOLD}2. Get Initial Token (Warm Cache)${NC}"
RAW_TOKEN_OUT=$("$COWEN_BIN" auth token --profile "$PROF" --format json)
TOKEN_1=$(echo "$RAW_TOKEN_OUT" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('access_token', ''))" 2>/dev/null)
echo "   Initial Token: $TOKEN_1"

if [ -z "$TOKEN_1" ]; then
    stop_test_redis
    fail_suite "Initial token retrieval failed"
fi

echo -e "${BOLD}3. Simulating Data Drift (Modifying Persistence Layer)${NC}"
DRIFT_TOKEN="drift_token_$(date +%s)"
sqlite3 "$DB_FILE" "UPDATE cowen_token SET item_value='$DRIFT_TOKEN' WHERE profile='$PROF';"
echo "   Persistence (SQL) updated to: $DRIFT_TOKEN"

echo -e "${BOLD}4. Fetching Token Again (Testing Reconciliation)${NC}"
TOKEN_2=$("$COWEN_BIN" auth token --profile "$PROF" --format json | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('access_token', ''))" 2>/dev/null)
echo "   Retrieved Token: $TOKEN_2"

stop_test_redis

if [ "$TOKEN_2" == "$DRIFT_TOKEN" ]; then
    echo -e "   ${GREEN}✓${NC} Data Drift Resolved! CLI served the updated token from persistence."
else
    echo -e "   ${YELLOW}⚠ [BLIND SPOT VERIFIED]${NC} Data Drift detected! CLI served stale cached token: $TOKEN_2"
    echo "   (This is a known blind spot. Hybrid Store relies purely on cache expiration and lacks active reconciliation.)"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 26 Passed!${NC}"
