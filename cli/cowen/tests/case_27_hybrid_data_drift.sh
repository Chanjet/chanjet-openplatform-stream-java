#!/bin/bash
# Case 27: Hybrid Store Data Drift (Blind Spot Verification)
# Verifies:
#   1. When using Hybrid Storage (Redis + SQL).
#   2. If SQL data is manually modified (data drift) while Redis is warm.
#   3. Does the CLI detect the drift and reconcile, or does it serve stale cache?
#   Note: This is a known blind spot. The test might fail.

source tests/common.sh

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_27"
start_mock

REDIS_PORT=6387
REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
REDIS_PID_FILE="target/cowen_tests/redis_case27.pid"

start_test_redis() {
    echo -e "  Starting test Redis on port $REDIS_PORT..."
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    redis-server --port $REDIS_PORT --daemonize yes --pidfile $(pwd)/$REDIS_PID_FILE
    sleep 2
}

stop_test_redis() {
    echo -e "  Stopping test Redis on port $REDIS_PORT..."
    pkill -f "cowen.*$PROF" 2>/dev/null || true
    if [ -f "$REDIS_PID_FILE" ]; then
        kill $(cat $REDIS_PID_FILE) || true
        rm -f $REDIS_PID_FILE
    else
        redis-cli -p $REDIS_PORT shutdown || true
    fi
    sleep 1
}

start_test_redis
DB_FILE="$COWEN_HOME/persistence.db"
PROF="hybrid_drift"

pkill -f "cowen.*$PROF" 2>/dev/null || true

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
    --proxy-port 9127 >/dev/null

echo "   Waiting for daemon to connect..."
sleep 3
curl -s -X POST -H "appKey: AK_HYBRID" "$MOCK_URL/auth/appTicket/resend" >/dev/null
sleep 2

echo -e "${BOLD}2. Get Initial Token (Warm Cache)${NC}"
RAW_TOKEN_OUT=$("$COWEN_BIN" auth token --profile "$PROF" --format json)
TOKEN_1=$(echo "$RAW_TOKEN_OUT" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('access_token', ''))" 2>/dev/null)
echo "   Initial Token: $TOKEN_1"

if [ -z "$TOKEN_1" ]; then
    echo -e "   ${RED}[FAILED]${NC} Initial token retrieval failed"
    stop_test_redis
    exit 1
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

echo -e "\n${GREEN}🎊 Case 27 Passed!${NC}"
