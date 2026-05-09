#!/bin/bash
# Case 17: Redis Shared Storage & Distributed Token Sync
# Verifies:
#   1. Node 1 and Node 2 can share tokens via Redis.
#   2. Token retrieved by Node 1 is immediately available to Node 2 without extra API calls.

source tests/e2e/scripts/common.sh

REDIS_PORT=6379
REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
REDIS_PID_FILE="$TEST_BASE/redis_case17.pid"

start_test_redis() {
    echo -e "  Starting isolated test Redis on port $REDIS_PORT..."
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    # --dir $COWEN_HOME: Isolates dump.rdb
    # --save "": Disables persistence to avoid IO race
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
    sleep 1
}

echo -e "${BOLD}1. Setup Redis and Node 1${NC}"
setup_workspace "case_17"
start_test_redis

# Define nodes
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
HOME_1="$TEST_BASE/.cowen_test_redis_node_1"
HOME_2="$TEST_BASE/.cowen_test_redis_node_2"

rm -rf "$HOME_1" "$HOME_2"
mkdir -p "$HOME_1" "$HOME_2"

clear_redis "$REDIS_URL"
start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: redis
  db_url: "$REDIS_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_REDIS \
    --app-secret AS_REDIS \
    --certificate CERT_REDIS \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port 9097 > /dev/null

assert_pass "Node 1 initialized with Redis storage"

# 2. Get Token on Node 1
echo -e "${BOLD}2. Get Token on Node 1${NC}"
TOKEN_1=$(extract_token "main")
if [[ -z "$TOKEN_1" ]]; then
    echo -e "   ${RED}[FAILED]${NC} Failed to get token on Node 1"
    stop_test_redis
    exit 1
fi
echo -e "   ✓ Node 1 Token: ${TOKEN_1:0:15}..."
echo "     Redis Keys:"
redis-cli -p $REDIS_PORT -n 0 KEYS "*"
sleep 2

# --- Node 2: Follower ---
echo -e "${BOLD}3. Setup Node 2 and Verify Token Sharing${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: redis
  db_url: "$REDIS_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Node 2 doesn't need to 'init' because it shares the same Redis DB
# But it needs the local app.yaml to know where Redis is.

# Verify Node 2 can see the same token
TOKEN_2=$(extract_token "main")

if [[ "$TOKEN_1" == "$TOKEN_2" ]]; then
    echo -e "   ${GREEN}✓${NC} Node 2 successfully retrieved the SAME token from Redis"
else
    echo -e "   ${RED}[FAILED]${NC} Token mismatch or sync failed"
    echo "     Node 1: $TOKEN_1"
    echo "     Node 2: $TOKEN_2"
    stop_test_redis
    exit 1
fi

# 4. Invalidate Token in Redis and Verify Renewal
echo -e "${BOLD}4. Invalidate Token in Redis and Verify Renewal${NC}"
# Clear Redis specifically for this key (Global App Profile with tok_v2: prefix)
redis-cli -p $REDIS_PORT -n 0 DEL "app:AK_REDIS:tok_v2:app_access" > /dev/null

TOKEN_3=$(extract_token "main")
echo "     TOKEN_1: $TOKEN_1"
echo "     TOKEN_3: $TOKEN_3"
if [[ "$TOKEN_3" != "$TOKEN_1" && -n "$TOKEN_3" ]]; then
    echo -e "   ${GREEN}✓${NC} Node 2 successfully renewed token after Redis deletion"
    echo "     New Token: ${TOKEN_3:0:15}..."
else
    echo -e "   ${RED}[FAILED]${NC} Token renewal failed or returned old token"
    stop_test_redis
    exit 1
fi

stop_test_redis
echo -e "\n${GREEN}🎊 Case 17 Passed!${NC}"
