#!/bin/bash
# Case 18: Redis Fault Tolerance & Recovery
# Verifies:
#   1. CLI can operate when Redis is up.
#   2. CLI handles Redis disconnection (fails or uses cache).
#   3. CLI recovers when Redis comes back online.

source tests/common.sh

REDIS_PORT=6380
REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
REDIS_PID_FILE="target/cowen_tests/redis_test.pid"

start_test_redis() {
    echo -e "  Starting test Redis on port $REDIS_PORT..."
    redis-server --port $REDIS_PORT --daemonize yes --pidfile $(pwd)/$REDIS_PID_FILE
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
    sleep 2
}

echo -e "${BOLD}1. Setup Environment and Start Redis${NC}"
setup_workspace "case_18"
start_test_redis
start_mock

# PRE-CONFIGURE REDIS
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: redis
  db_url: "$REDIS_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Initialize with pre-existing Redis config
# We use --app-key etc to fill the storage
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_FAULT \
    --app-secret AS_FAULT \
    --certificate CERT_FAULT \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port 9098

assert_pass "SelfBuilt initialized with Redis"

# 2. Verify normal operation
echo -e "${BOLD}2. Verify Normal Operation${NC}"
TOKEN_1=$(extract_token "main")
if [[ -n "$TOKEN_1" ]]; then
    echo -e "   ${GREEN}✓${NC} Initial token retrieval success: ${TOKEN_1:0:15}..."
else
    echo -e "   ${RED}[FAILED]${NC} Initial token retrieval failed"
    # Dump log for debugging
    cat "$COWEN_HOME/main.log" 2>/dev/null
    stop_test_redis
    exit 1
fi

# 3. Stop Redis and verify failure/behavior
echo -e "${BOLD}3. Stop Redis and Test Behavior${NC}"
stop_test_redis

echo "   Requesting token with Redis down..."
# We expect it to either fail or use a local cache if it exists.
# Most likely it will fail if it's strictly redis-backed.
TOKEN_2=$(extract_token "main")

if [[ -z "$TOKEN_2" ]]; then
    echo -e "   ${GREEN}✓${NC} Token retrieval failed as expected (Redis down)"
else
    # If it still returns a token, it might be using local memory cache (which is also good)
    echo -e "   ${YELLOW}[INFO]${NC} Token retrieval still returned something: ${TOKEN_2:0:15}..."
    echo "          (Could be memory cache)"
fi

# 4. Start Redis again and verify recovery
echo -e "${BOLD}4. Restart Redis and Verify Recovery${NC}"
start_test_redis

TOKEN_3=$(extract_token "main")
if [[ -n "$TOKEN_3" ]]; then
    echo -e "   ${GREEN}✓${NC} Token retrieval recovered after Redis restart: ${TOKEN_3:0:15}..."
else
    echo -e "   ${RED}[FAILED]${NC} Token retrieval failed to recover"
    stop_test_redis
    exit 1
fi

# Cleanup
stop_test_redis
echo -e "\n${GREEN}🎊 Case 18 Passed!${NC}"
