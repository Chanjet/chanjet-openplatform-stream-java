#!/bin/bash
set -e
# Case 35: StoreApp AppTicket Storage Persistence in PostgreSQL
# Verifies:
#   1. When an AppTicket is received from the platform (Mock), it is saved in PostgreSQL.
#   2. The ticket is persisted and survives daemon restarts.
#   3. Multiple nodes can access the same AppTicket from shared storage.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

# Configuration
DB_NAME=$(get_case_db_name "case_35")

echo -e "${BOLD}1. Setup PostgreSQL and StoreApp Node 1${NC}"
setup_workspace "case_35"
PG_URL=$(setup_postgres_db "$DB_NAME")

HOME_1="$COWEN_HOME/node_1"
HOME_2="$COWEN_HOME/node_2"
mkdir -p "$HOME_1" "$HOME_2"

start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: postgres
  db_url: "$PG_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Initialize as StoreApp
APP_KEY="AK_PG_STORE"
"$COWEN_BIN" init --profile main \
    --app-mode store-app \
    --app-key $APP_KEY \
    --app-secret "AS_PG_STORE" \
    --encrypt-key "1234567890123456" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port $PROXY_PORT > /dev/null

echo -e "   ✓ Node 1 initialized as StoreApp"

echo -e "${BOLD}2. Trigger AppTicket Push and Verify Storage${NC}"

# Start daemon on Node 1 to receive ticket
"$COWEN_BIN" daemon start --profile main >/dev/null
wait_for_daemon main 10

echo -e "  Triggering AppTicket push for Node 1..."
curl -s -X POST -H "appKey: $APP_KEY" "$MOCK_URL/auth/appTicket/resend" >/dev/null

# Verify ticket exists in PostgreSQL with retries
echo -n "  Verifying AppTicket in PostgreSQL..."
TICKET_IN_DB=""
for i in {1..10}; do
    TICKET_IN_DB=$(safe_psql_exec "SELECT ticket_value FROM cowen_ticket WHERE app_key = '$APP_KEY';" "$DB_NAME" | grep -v "ticket_value" | grep -v "\-\-\-" | grep -v "rows" | xargs)
    if [[ -n "$TICKET_IN_DB" ]]; then
        break
    fi
    sleep 1
done

if [[ -n "$TICKET_IN_DB" ]]; then
    echo -e " ${GREEN}[OK]${NC} (Value found: ${TICKET_IN_DB:0:15}...)"
else
    echo -e " ${RED}[FAILED]${NC} AppTicket not found in cowen_ticket table"
    "$COWEN_BIN" daemon stop --profile main
    exit 1
fi

echo -e "${BOLD}3. Verify Persistence after Node 1 Restart${NC}"
"$COWEN_BIN" daemon stop --profile main
sleep 1

echo -n "  Verifying AppTicket persists after daemon stop..."
TICKET_AFTER_STOP=$(safe_psql_exec "SELECT ticket_value FROM cowen_ticket WHERE app_key = '$APP_KEY';" "$DB_NAME" | grep -v "ticket_value" | grep -v "\-\-\-" | grep -v "rows" | xargs)
if [[ "$TICKET_IN_DB" == "$TICKET_AFTER_STOP" ]]; then
    echo -e " ${GREEN}[OK]${NC}"
else
    echo -e " ${RED}[FAILED]${NC} Ticket lost or changed after stop"
    exit 1
fi

echo -e "${BOLD}4. Verify Node 2 Access (Shared Storage)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: postgres
  db_url: "$PG_URL"
log:
  level: debug
openapi_url: "$MOCK_URL"
stream_url: "$MOCK_WS"
telemetry_enabled: false
ai_enabled: false
EOF

# Node 2 should be able to get the token immediately because the ticket is already in PG
# It will use the ticket from PG to exchange for a token from the mock platform
TOKEN_2=""
for i in {1..10}; do
    TOKEN_2=$(extract_token "main")
    if [[ -n "$TOKEN_2" ]]; then
        break
    fi
    sleep 1
done

if [[ -n "$TOKEN_2" ]]; then
    echo -e "   ✓ Node 2 successfully used shared AppTicket from PG to acquire token"
else
    echo -e "   ${RED}[FAILED]${NC} Node 2 could not acquire token using shared ticket"
    exit 1
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 35 Passed!${NC}"
cleanup_suite
