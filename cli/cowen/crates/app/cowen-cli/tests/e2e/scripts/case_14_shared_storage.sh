#!/bin/bash
set -e
# Case 14: Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 2 can start without 'init' by reading config from a shared DB initialized by Node 1.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via shared storage.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_14"

HOME_1="$COWEN_HOME/node_1"
HOME_2="$COWEN_HOME/node_2"
SHARED_DB="$COWEN_HOME/shared_storage_case_14.db"

# 🚀 Dynamic Ports
PROXY_PORT_1=$(get_unused_port)
PROXY_PORT_2=$(get_unused_port)

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 14 environment...${NC}"
    kill_daemons_in_dirs "$HOME_1" "$HOME_2"
    cleanup_suite
}
trap final_cleanup EXIT

mkdir -p "$HOME_1" "$HOME_2"


start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$SHARED_DB"
log:
  level: debug
openapi_url: "$MOCK_URL"
stream_url: "$MOCK_WS"
telemetry_enabled: false
ai_enabled: false
EOF


"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SYNC \
    --app-secret AS_SYNC \
    --certificate CERT_SYNC \
    --encrypt-key 1234567890123456 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --webhook-target "$MOCK_URL/webhook_sink"


# Daemon is auto-started by init
wait_for_daemon "main" 10
sleep 2

# --- Node 2: Follower (No Init) ---
echo -e "${BOLD}2. Setup Node 2 (No Init)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$SHARED_DB"
log:
  level: debug
openapi_url: "$MOCK_URL"
stream_url: "$MOCK_WS"
telemetry_enabled: false
ai_enabled: false
EOF

# Start daemon on Node 2 so it can query the shared DB
"$COWEN_BIN" daemon start --profile default --foreground > "$HOME_2/daemon.log" 2>&1 < /dev/null &
NODE2_PID=$!
sleep 2

# Node 2 should see 'main' profile via daemon status
PROFILES=$("$COWEN_BIN" status -a 2>/dev/null)
if [[ "$PROFILES" == *"main"* ]]; then
    echo -e "   ✓ Node 2 successfully discovered 'main' profile from shared DB"
else
    fail_suite "Node 2 could not see 'main' profile"
fi

# 3. Verify Token Synchronization
echo -e "${BOLD}3. Verify Token Synchronization${NC}"

# 1. Get initial token from Node 1
export COWEN_HOME="$HOME_1"
echo -n "   Extracting token from Node 1..."
TOKEN_1=$(wait_for_token "main" "" 15)
if [ -z "$TOKEN_1" ]; then
    fail_suite "Node 1 token extraction failed."
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
    # Force Node 2 daemon to reload token from shared storage
    "$COWEN_BIN" daemon reload --profile main > /dev/null
    sleep 5
done

if [ "$TOKEN_1" != "$TOKEN_2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    echo "     Node 1: $TOKEN_1"
    fail_suite "Node 2: $TOKEN_2"
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$(extract_token "main" "--refresh")
if [ -z "$TOKEN_V2" ]; then
    fail_suite "Node 1 refresh failed"
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
    fail_suite "Node 2: $TOKEN_2_V2"
fi

echo -e "${BOLD}6. Verify Node 2 Proxy Implementation${NC}"
export COWEN_HOME="$HOME_2"
# Stop any background daemon auto-launched by Node 2 commands earlier, to ensure clean port binding and socket ownership
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
# Start Node 2 daemon to test proxy on port PROXY_PORT_2
"$COWEN_BIN" daemon start --profile main --proxy-port $PROXY_PORT_2 --foreground > "$HOME_2/daemon.log" 2>&1 < /dev/null &

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
    cat "$HOME_2/daemon.log"
    fail_suite "--- Daemon Log ---"
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
    fail_suite "Actual token in header:   $LAST_TOKEN"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

pass_suite
