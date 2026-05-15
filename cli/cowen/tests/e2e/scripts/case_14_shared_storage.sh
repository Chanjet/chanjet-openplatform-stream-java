#!/bin/bash
# Case 14: Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 2 can start without 'init' by reading config from a shared DB initialized by Node 1.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via shared storage.

source tests/e2e/scripts/common.sh

# Force cleanup before starting
# pkill -9 cowen-test || true
sleep 1

echo -e "${BOLD}1. Setup Shared Storage and Node 1${NC}"

# Define nodes
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
HOME_1="$TEST_BASE/.cowen_test_dist_sync_node_1"
HOME_2="$TEST_BASE/.cowen_test_dist_sync_node_2"
# 🚀 Fix: Use a name that doesn't trigger the '.cowen_test_' regex replacement in run_parallel.sh
# This ensures Node 1 and Node 2 definitely use the same filename.
SHARED_DB_NAME="shared_storage_case_14.db"
SHARED_DB="$TEST_BASE/$SHARED_DB_NAME"
mkdir -p "$TEST_BASE"

# 🚀 Dynamic Ports
PROXY_PORT_1=$(get_unused_port)
PROXY_PORT_2=$(get_unused_port)

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 14 environment...${NC}"
    cleanup_suite
    rm -rf "$HOME_1" "$HOME_2"
}
# trap final_cleanup EXIT

rm -f "$SHARED_DB"*
mkdir -p "$HOME_1" "$HOME_2"

start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: innerdb
  db_url: "sqlite://$SHARED_DB"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SYNC \
    --app-secret AS_SYNC \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SYNC \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "http://127.0.0.1:9299/webhook_sink" \
    --proxy-port $PROXY_PORT_1
assert_pass "Node 1 initialized and linked to shared DB ($SHARED_DB_NAME)"

# --- Node 2: Follower (No Init) ---
echo -e "${BOLD}2. Setup Node 2 (No Init)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: innerdb
  db_url: "sqlite://$SHARED_DB"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Node 2 should see 'main' profile immediately
PROFILES=$("$COWEN_BIN" profile list)
if [[ "$PROFILES" == *"main"* ]]; then
    echo -e "   ✓ Node 2 successfully discovered 'main' profile from shared DB"
else
    echo -e "   ${RED}[FAILED]${NC} Node 2 could not see 'main' profile"
    exit 1
fi

# 3. Verify Token Synchronization
echo -e "${BOLD}3. Verify Token Synchronization${NC}"

# 1. Get initial token from Node 1
export COWEN_HOME="$HOME_1"
echo -n "   Extracting token from Node 1..."
TOKEN_1=$(extract_token "main")
if [ -z "$TOKEN_1" ]; then
    echo -e " ${RED}[FAILED]${NC} Node 1 token extraction failed."
    exit 1
fi
echo -e " ${GREEN}[OK]${NC} (${TOKEN_1:0:15}...)"

# 2. Get token from Node 2 (should read from DB)
# 🚀 Fix: Added retry loop for shared SQLite storage propagation in QEMU/Linux
echo -n "   Extracting token from Node 2..."
TOKEN_2=""
for i in {1..10}; do
    export COWEN_HOME="$HOME_2"
    TOKEN_2=$(extract_token "main")
    if [ "$TOKEN_1" == "$TOKEN_2" ]; then
        echo -e " ${GREEN}[OK]${NC} (${TOKEN_2:0:15}...)"
        break
    fi
    echo -n "."
    sleep 1
done

if [ "$TOKEN_1" != "$TOKEN_2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    echo "     Node 1: $TOKEN_1"
    echo "     Node 2: $TOKEN_2"
    exit 1
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$(extract_token "main" "--refresh")
if [ -z "$TOKEN_V2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Node 1 refresh failed"
    exit 1
fi
echo -e "   Node 1 New Token:     ${BLUE}${TOKEN_V2:0:15}...${NC}"

# 🚀 STABILITY: Stop Node 1 daemon so it doesn't background-refresh again and change the token
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true

# 🚀 Fix: Added retry loop for shared SQLite storage propagation in QEMU/Linux
echo -n "   Node 2 fetching token from shared SQLite..."
TOKEN_2_V2=""
for i in {1..10}; do
    export COWEN_HOME="$HOME_2"
    TOKEN_2_V2=$(extract_token "main")
    if [ "$TOKEN_V2" == "$TOKEN_2_V2" ]; then
        echo -e " ${GREEN}[OK]${NC}"
        break
    fi
    echo -n "."
    sleep 1
done

if [ "$TOKEN_V2" != "$TOKEN_2_V2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Node 2 token not synchronized after refresh"
    echo "     Node 1: $TOKEN_V2"
    echo "     Node 2: $TOKEN_2_V2"
    exit 1
fi

echo -e "${BOLD}6. Verify Node 2 Proxy Implementation${NC}"
export COWEN_HOME="$HOME_2"
# Start Node 2 daemon to test proxy on port PROXY_PORT_2
"$COWEN_BIN" daemon start --profile main --proxy-port $PROXY_PORT_2 --foreground > "$HOME_2/daemon.log" 2>&1 &
echo -n "   Waiting for Node 2 daemon to stabilize..."
sleep 10

echo -n "   Verifying Node 2 Proxy uses new token..."
# We use curl with a retry because the daemon might take a moment to bind
MAX_RETRIES=15
SUCCESS=0
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s -f -X POST -d '{"test":true}' -x "http://127.0.0.1:$PROXY_PORT_2" "$MOCK_URL/webhook_sink" > /dev/null; then
        SUCCESS=1
        break
    fi
    echo -n "."
    sleep 2
done

if [ $SUCCESS -eq 0 ]; then
    echo -e " ${RED}[FAILED]${NC} Node 2 Proxy unreachable after $MAX_RETRIES attempts"
    echo "  --- Daemon Log ---"
    cat "$HOME_2/daemon.log"
    exit 1
fi

RAW_CONTROL=$(curl -s "$MOCK_URL/control/webhooks")
echo "DEBUG: Raw Control Webhooks: $RAW_CONTROL"
# 🚀 Fix: Find the first message that HAS a token header, instead of assuming it is the last message
LAST_TOKEN=$(echo "$RAW_CONTROL" | python3 -c "import sys, json; d=json.load(sys.stdin); 
found='';
for msg in d:
    h = msg.get('headers', {})
    t = h.get('openToken') or h.get('opentoken')
    if t: found = t
print(found)" 2>/dev/null)

if [[ "$LAST_TOKEN" == *"$TOKEN_V2"* ]]; then
    echo -e " ${GREEN}[VERIFIED]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    echo "     Expected token in header: $TOKEN_V2"
    echo "     Actual token in header:   $LAST_TOKEN"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 14 Passed!${NC}"
