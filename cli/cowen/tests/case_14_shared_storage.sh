#!/bin/bash
# Case 14: Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 2 can start without 'init' by reading config from a shared DB initialized by Node 1.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via shared storage.

source tests/common.sh

# Force cleanup before starting
pkill -9 cowen || true
sleep 1

echo -e "${BOLD}1. Setup Shared Storage and Node 1${NC}"

HOME_1="$(pwd)/.cowen_test_dist_sync_node_1"
HOME_2="$(pwd)/.cowen_test_dist_sync_node_2"
SHARED_DB="$(pwd)/.cowen_test_shared.db"

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 14 environment...${NC}"
    cleanup_suite
    rm -rf "$HOME_1" "$HOME_2"
}
trap final_cleanup EXIT

rm -rf "$HOME_1" "$HOME_2"
rm -f "$SHARED_DB"* "shared_cowen.db"*
mkdir -p "$HOME_1" "$HOME_2"

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite:///$SHARED_DB"
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
    --proxy-port 9093

# Stop ghost daemon
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
assert_pass "Node 1 initialized and linked to shared DB"

# --- Node 2: Follower (No Init) ---
echo -e "${BOLD}2. Setup Node 2 (No Init)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite:///$SHARED_DB"
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

start_mock

# 3. Verify Token Synchronization
echo -e "${BOLD}3. Verify Token Synchronization${NC}"

# 1. Get initial token from Node 1
export COWEN_HOME="$HOME_1"
TOKEN_1=$("$COWEN_BIN" auth token --profile main --format json | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token'))")
echo -e "   Node 1 Initial Token: ${BLUE}${TOKEN_1:0:15}...${NC}"

# 2. Get token from Node 2 (should read from DB)
export COWEN_HOME="$HOME_2"
TOKEN_2=$("$COWEN_BIN" auth token --profile main --format json | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token'))")
echo -e "   Node 2 Initial Token: ${BLUE}${TOKEN_2:0:15}...${NC}"

if [ "$TOKEN_1" == "$TOKEN_2" ]; then
    echo -e "   ✓ Initial token synchronized via shared DB"
else
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    exit 1
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$("$COWEN_BIN" auth token --profile main --refresh --format json | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token'))")
echo -e "   Node 1 New Token:     ${BLUE}${TOKEN_V2:0:15}...${NC}"

echo -e "${BOLD}5. Verify Node 2 Sync${NC}"
export COWEN_HOME="$HOME_2"
TOKEN_2_V2=$("$COWEN_BIN" auth token --profile main --format json | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token'))")
echo -e "   Node 2 New Token:     ${BLUE}${TOKEN_2_V2:0:15}...${NC}"

if [ "$TOKEN_V2" == "$TOKEN_2_V2" ]; then
    echo -e "   ✓ Node 2 picked up refreshed token from Node 1 via DB"
else
    echo -e "   ${RED}[FAILED]${NC} Node 2 token not synchronized after refresh"
    echo "     Node 1: $TOKEN_V2"
    echo "     Node 2: $TOKEN_2_V2"
    exit 1
fi

echo -e "${BOLD}6. Verify Node 2 Proxy Implementation${NC}"
export COWEN_HOME="$HOME_2"
# Start Node 2 daemon to test proxy on port 9094
"$COWEN_BIN" daemon start --profile main --proxy-port 9094 --foreground > "$HOME_2/daemon.log" 2>&1 &
sleep 2

echo -n "   Verifying Node 2 Proxy uses new token..."
# We use curl with a retry because the daemon might take a moment to bind
MAX_RETRIES=5
SUCCESS=0
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s -f -X POST -d '{"test":true}' -x "http://127.0.0.1:9094" "$MOCK_URL/webhook_sink" > /dev/null; then
        SUCCESS=1
        break
    fi
    sleep 1
done

if [ $SUCCESS -eq 0 ]; then
    echo -e " ${RED}[FAILED]${NC} Node 2 Proxy unreachable"
    cat "$HOME_2/daemon.log"
    exit 1
fi

RAW_CONTROL=$(curl -s "$MOCK_URL/control/webhooks")
echo "DEBUG: Raw Control Webhooks: $RAW_CONTROL"
LAST_TOKEN=$(echo "$RAW_CONTROL" | python3 -c "import sys, json; d=json.load(sys.stdin); print(d[-1].get('headers', {}).get('Authorization', ''))" 2>/dev/null)

if [[ "$LAST_TOKEN" == *"$TOKEN_V2"* ]]; then
    echo -e " ${GREEN}[VERIFIED]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    echo "     Expected token in header: $TOKEN_V2"
    echo "     Actual token in header:   $LAST_TOKEN"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 14 Passed!${NC}"
