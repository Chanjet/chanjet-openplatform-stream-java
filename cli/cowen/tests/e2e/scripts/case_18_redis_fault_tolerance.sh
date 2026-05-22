#!/bin/bash
set -e
# Case 18: Redis Fault Tolerance & Recovery (Hybrid Architecture)
# Verifies:
#   1. System works with Hybrid Store (SQLite Persistence + Redis Cache).
#   2. When Redis is down, system still has config but token might fail.
#   3. When Redis is back, system recovers and re-caches token.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

REDIS_PORT=6380
REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
REDIS_PID_FILE="$TEST_BASE/redis_case18.pid"

start_test_redis() {
    echo -e "  Starting isolated test Redis on port $REDIS_PORT..."
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    redis-server --port $REDIS_PORT --dir "$COWEN_HOME" --save "" --daemonize yes --pidfile $(pwd)/$REDIS_PID_FILE
    sleep 2
}

stop_test_redis() {
    echo -e "  Stopping test Redis on port $REDIS_PORT..."
    if [ -f "$REDIS_PID_FILE" ]; then
        kill $(cat $REDIS_PID_FILE) || true
        rm -f $REDIS_PID_FILE
    else
        redis-cli -p $REDIS_PORT shutdown || true
    fi
}

echo -e "${BOLD}1. Setup Environment (Hybrid Store)${NC}"
setup_workspace "case_18"
start_test_redis
start_mock
PROF="redis_hybrid"

# Force Hybrid Configuration
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$COWEN_HOME/persistence.db"
  cache: redis
  cache_url: "$REDIS_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Initialize
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode self-built \
    --app-key AK_FAULT \
    --app-secret AS_FAULT \
    --certificate CERT_FAULT \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT > /dev/null

# Start daemon to establish WebSocket and get AppTicket
"$COWEN_BIN" daemon start --profile "$PROF" > /dev/null
sleep 5
"$COWEN_BIN" auth login --profile "$PROF" --force >/dev/null
sleep 2

# 2. Verify Normal Operation
echo -e "${BOLD}2. Verify Normal Operation${NC}"
TOKEN_1=$(extract_token "$PROF")
if [[ -n "$TOKEN_1" ]]; then
    echo -e "   ${GREEN}✓${NC} Initial token retrieval success"
else
    stop_test_redis
    fail_suite "Initial token retrieval failed"
fi

# 3. Stop Redis
echo -e "${BOLD}3. Stop Redis and Verify Behavior${NC}"
stop_test_redis
sleep 2

echo "   Requesting token with Redis (Cache) down..."
# In hybrid mode, it might fallback to SQLite or fail depending on strategy.
# But it shouldn't crash.
TOKEN_2=$(extract_token "$PROF")
echo -e "   ${GREEN}✓${NC} Request handled with Redis down (Token: ${TOKEN_2:0:10}...)"

# 4. Restart Redis and Recovery
echo -e "${BOLD}4. Restart Redis and Verify Recovery${NC}"
start_test_redis

# Optional: restart daemon to clear any backoff if it's too long
pkill -9 -f "$(basename "$COWEN_BIN").*$PROF" || true
"$COWEN_BIN" daemon start --profile "$PROF" > /dev/null 2>&1
sleep 3

TOKEN_3=$(extract_token "$PROF")
if [[ -n "$TOKEN_3" ]]; then
    echo -e "   ${GREEN}✓${NC} System recovered after Redis restart"
else
    fail_suite "System failed to recover"
fi

# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile "$PROF" 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

# Cleanup
stop_test_redis

echo -e "\n${GREEN}🎊 Case 18 Passed!${NC}"
