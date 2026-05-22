#!/bin/bash
set -e
# Case 15: StoreApp Mode - Shared Storage & Distributed Sync
# Verifies:
#   1. Node 2 can start in Sidecar (store-app) mode by reading credentials from shared DB.
#   2. Webhook events (APP_TICKET) received by Node 2 are stored in shared DB and visible to Node 1.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
PROXY_PORT_2=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_15"

HOME_1="$COWEN_HOME/node_1"
HOME_2="$COWEN_HOME/node_2"
SHARED_DB="$COWEN_HOME/store_app_shared.db"

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 15 environment...${NC}"
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
    --app-mode store-app \
    --app-key AK_STORE \
    --app-secret AS_STORE \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "http://127.0.0.1:9299/webhook_sink" \
    --proxy-port $PROXY_PORT

assert_pass "Node 1 initialized with StoreApp mode in shared DB"

# --- Node 2: Follower ---
echo -e "${BOLD}2. Setup Node 2 and Start Daemon${NC}"
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



# Node 2 starts daemon
"$COWEN_BIN" daemon start --profile main --proxy-port $PROXY_PORT_2 --foreground > "$HOME_2/daemon.log" 2>&1 &
DAEMON_PID=$!
sleep 2

# Verify Node 2 is running
if ! ps -p $DAEMON_PID > /dev/null; then
    echo -e "   ${RED}[FAILED]${NC} Node 2 daemon failed to start"
    cat "$HOME_2/daemon.log"
    exit 1
fi
echo -e "   ✓ Node 2 daemon started successfully"


# 3. Simulate Platform Webhook to Node 2
echo -e "${BOLD}3. Simulate Platform Webhook to Node 2${NC}"
TICKET_VAL="mock_ticket_$(date +%s)"
curl -s -X POST -H "Content-Type: application/json" \
     -d "{\"type\":\"APP_TICKET\", \"app_ticket\":\"$TICKET_VAL\"}" \
     "http://127.0.0.1:$PROXY_PORT_2/webhook" > /dev/null

echo -e "   ✓ Sent APP_TICKET to Node 2 Proxy"
sleep 5

# 4. Verify Ticket Synchronization
echo -e "${BOLD}4. Verify Ticket Synchronization${NC}"
# Check if Node 1 can see the ticket from shared DB with retries
export COWEN_HOME="$HOME_1"
ACTUAL_TICKET=""
for i in {1..10}; do
    ACTUAL_TICKET=$("$COWEN_BIN" auth status --profile main --format json | python3 -c "import sys, json; 
data = json.load(sys.stdin);
def find_entry(entries, name):
    for e in entries:
        if e['name'] == name: return e
        res = find_entry(e.get('children', []), name)
        if res: return res
    return None
entries = []
if 'profiles' in data and len(data['profiles']) > 0:
    entries = data['profiles'][0].get('entries', [])
elif 'entries' in data:
    entries = data.get('entries', [])
ticket = find_entry(entries, 'AppTicket');
print(ticket['message'] if ticket else '')")
    
    if [[ "$ACTUAL_TICKET" == *"[CACHED]"* ]]; then
        break
    fi
    echo -e "  [WAIT] Waiting for Ticket to propagate to Node 1 (Attempt $i/10)..."
    sleep 2
done

if [[ "$ACTUAL_TICKET" == *"[CACHED]"* ]]; then
    echo -e "   ${GREEN}✓${NC} Node 1 successfully verified Ticket synchronized from Node 2 (Status: [CACHED])"
else
    echo -e "   ${RED}[FAILED]${NC} Ticket synchronization failed"
    echo "     Expected status containing: [CACHED]"
    echo "     Actual status message:     $ACTUAL_TICKET"
    echo "     --- RAW STATUS OUTPUT ---"
    "$COWEN_BIN" auth status --profile main --format json 2>&1
    echo "     -------------------------"
    exit 1
fi

# 5. Verify AppAccessToken Generation on Node 2
echo -e "${BOLD}5. Verify AppAccessToken Generation on Node 2${NC}"
export COWEN_HOME="$HOME_2"
RAW_TOKEN=$("$COWEN_BIN" auth token --profile main --format json)
TOKEN=$(get_json_field "$RAW_TOKEN" "access_token")

if [[ "$TOKEN" == mock_at_sa_* ]]; then
    echo -e "   ${GREEN}✓${NC} Node 2 successfully generated AppAccessToken using shared credentials"
    echo "     Token: $TOKEN"
else
    echo -e "   ${RED}[FAILED]${NC} Token generation failed on Node 2"
    "$COWEN_BIN" auth status --profile main
    exit 1
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

pass_suite
