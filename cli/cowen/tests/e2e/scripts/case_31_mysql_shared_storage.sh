#!/bin/bash
set -e
# Case 31: MySQL Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 1 and Node 2 can share tokens via MySQL.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via MySQL.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

# Configuration
DB_NAME=$(get_case_db_name "case_31")

echo -e "${BOLD}1. Setup MySQL Isolation and Node 1${NC}"
setup_workspace "case_31"
MYSQL_URL=$(setup_mysql_db "$DB_NAME")


# Define nodes
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
HOME_1="$TEST_BASE/.cowen_test_mysql_node_1"
HOME_2="$TEST_BASE/.cowen_test_mysql_node_2"

rm -rf "$HOME_1" "$HOME_2"
mkdir -p "$HOME_1" "$HOME_2"

start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: mysql
  db_url: "$MYSQL_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Node 1 initialization will now happen in a completely fresh database
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_MYSQL \
    --app-secret AS_MYSQL \
    --encrypt-key 1234567890123456 \
    --certificate CERT_MYSQL \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port $PROXY_PORT > /dev/null

# Start daemon
"$COWEN_BIN" daemon start --profile main > /dev/null

# Proactively login with retries to handle WebSocket connection latency
echo "  Logging in and waiting for AppTicket sync..."
SUCCESS=false
INITIAL_TOKEN=""
for attempt in {1..5}; do
    "$COWEN_BIN" auth login --profile main --force >/dev/null 2>&1 || true
    sleep 2
    
    INITIAL_TOKEN=$(extract_token "main")
    if [[ -n "$INITIAL_TOKEN" && "$INITIAL_TOKEN" != *"Authentication failed"* ]]; then
        SUCCESS=true
        break
    fi
    echo "  [WAIT] WebSocket sync not yet completed, retrying login (Attempt $attempt/5)..."
    sleep 2
done

if [ "$SUCCESS" != "true" ]; then
    fail_suite "Node 1 daemon failed to sync AppTicket after multiple attempts"
fi

assert_pass "Node 1 initialized and linked to MySQL"

# --- Node 2: Follower ---
echo -e "${BOLD}2. Setup Node 2 (No Init)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: mysql
  db_url: "$MYSQL_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Start daemon on Node 2 so it can query the shared DB
"$COWEN_BIN" daemon start --profile default --foreground > "$HOME_2/daemon.log" 2>&1 &
PID_2=$!
sleep 1

# Node 2 should see 'main' profile immediately
PROFILES=$("$COWEN_BIN" status --all)
if [[ "$PROFILES" == *"main"* ]]; then
    echo -e "   ✓ Node 2 successfully discovered 'main' profile from MySQL"
else
    fail_suite "Node 2 could not see 'main' profile"
fi

# 3. Verify Token Synchronization
echo -e "${BOLD}3. Verify Token Synchronization${NC}"

# 1. Get initial token from Node 1
export COWEN_HOME="$HOME_1"
TOKEN_1="$INITIAL_TOKEN"
if [[ -z "$TOKEN_1" || "$TOKEN_1" == *"Authentication failed"* ]]; then
    TOKEN_1=$(wait_for_token "main" "tok_")
fi
assert_sanitized "$TOKEN_1" "Node 1 Initial Token sanitization"
echo -e "   Node 1 Initial Token: ${BLUE}${TOKEN_1:0:15}...${NC}"

# 2. Get token from Node 2 (should read from DB)
# 🚀 Fix: Added retry loop for shared storage propagation in QEMU/Linux
echo -n "   Node 2 fetching token from shared MySQL..."
TOKEN_2=""
for i in {1..10}; do
    export COWEN_HOME="$HOME_2"
    TOKEN_2=$(extract_token "main")
    if [ "$TOKEN_1" == "$TOKEN_2" ]; then
        echo -e " ${GREEN}[OK]${NC}"
        break
    fi
    echo -n "."
    sleep 1
done

if [ "$TOKEN_1" != "$TOKEN_2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    echo "     Node 1: $TOKEN_1"
    fail_suite "Node 2: $TOKEN_2"
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$(extract_token "main" --refresh)
assert_sanitized "$TOKEN_V2" "Node 1 Refreshed Token sanitization"
echo -e "   Node 1 New Token:     ${BLUE}${TOKEN_V2:0:15}...${NC}"

echo -e "${BOLD}5. Verify Node 2 Sync${NC}"
export COWEN_HOME="$HOME_2"
TOKEN_2_V2=$(extract_token "main")
assert_sanitized "$TOKEN_2_V2" "Node 2 Synced Token sanitization"
echo -e "   Node 2 New Token:     ${BLUE}${TOKEN_2_V2:0:15}...${NC}"

if [ "$TOKEN_V2" == "$TOKEN_2_V2" ]; then
    echo -e "   ✓ Node 2 picked up refreshed token from Node 1 via MySQL"
else
    export COWEN_HOME="$HOME_1"
    cleanup_suite
    export COWEN_HOME="$HOME_2"
    cleanup_suite
    fail_suite "Node 2 token not synchronized after refresh"
fi

export COWEN_HOME="$HOME_1"
cleanup_suite
export COWEN_HOME="$HOME_2"
cleanup_suite

echo -e "\n${GREEN}🎊 Case 31 Passed!${NC}"

