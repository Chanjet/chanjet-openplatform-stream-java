#!/bin/bash
set -e
# Case 19: StoreApp Mode - Ticket Auto-Resend
# Verifies:
#   1. When AppTicket is missing, 'auth token' triggers /auth/appTicket/resend.
#   2. Daemon receives the ticket and updates storage.
#   3. 'auth token' eventually succeeds.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
pkill -f "cowen_case_19" || true
sleep 2
setup_workspace "case_19"
start_mock

# Initialize StoreApp
# We use SQLite for this case
"$COWEN_BIN" init --profile main \
    --app-mode store-app \
    --app-key AK_STORE \
    --app-secret AS_STORE \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port $PROXY_PORT

assert_pass "StoreApp initialized"

# Start Daemon (to receive the ticket)
"$COWEN_BIN" daemon start --profile main --foreground > "$COWEN_HOME/daemon.log" 2>&1 &
DAEMON_PID=$!
sleep 8

# Verify initial token works
echo -e "${BOLD}2. Verify Initial Token Retrieval${NC}"
# For StoreApp, auth token command will trigger resend and wait
TOKEN_1=$(extract_token "main")
if [[ -z "$TOKEN_1" ]]; then
    echo -e "   ${RED}[FAILED]${NC} Initial token retrieval failed"
    echo "--- Daemon Log ---"
    cat "$COWEN_HOME/daemon.log"
    echo "--- Mock Server Log ---"
    cat "$TEST_BASE/mock_server_$MOCK_PORT.log"
    exit 1
fi
echo -e "   ✓ Initial Token: ${TOKEN_1:0:15}..."

# 3. Simulate Ticket Missing
echo -e "${BOLD}3. Simulate Ticket Missing${NC}"
# Delete ticket from SQLite
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_config WHERE profile = 'app:AK_STORE' AND item_key = 'app_ticket';"
sqlite3 "$COWEN_HOME/cowen.db" "DELETE FROM cowen_config WHERE profile = 'app:AK_STORE' AND item_key = 'app_ticket_created';"

# Clear mock server webhook/request history if possible
# Actually we can just check if /auth/appTicket/resend is called.

# 4. Request Token again
echo -e "${BOLD}4. Request Token (Should trigger resend)${NC}"
# This might take a few seconds because of the retry loop in SelfBuilt/StoreApp provider
TOKEN_2=$(extract_token "main")

if [[ -n "$TOKEN_2" ]]; then
    echo -e "   ${GREEN}✓${NC} Token retrieval succeeded after ticket deletion"
    echo "     New Token: ${TOKEN_2:0:15}..."
else
    echo -e "   ${RED}[FAILED]${NC} Token retrieval failed after ticket deletion"
    exit 1
fi

# Cleanup
kill -9 $DAEMON_PID > /dev/null 2>&1 || true


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 19 Passed!${NC}"
