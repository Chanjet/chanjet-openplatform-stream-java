#!/bin/bash
set -e
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)

# Aggressive cleanup of any leftover cowen processes from previous failed runs
pkill -9 -f "cowen_store_app_activation" || true
sleep 1

setup_workspace "case_36"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile sidecar \
    --app-mode store-app \
    --app-key AK_SA \
    --app-secret AS_SA \
    --encrypt-key 1234567890123456 \
    --webhook-target "http://127.0.0.1:$PROXY_PORT/webhook" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Daemon Startup${NC}"
# Stop the background daemon and wait for port release
"$COWEN_BIN" daemon stop --profile sidecar >/dev/null 2>&1 || true
sleep 1
# Ensure port $PROXY_PORT is REALLY free
lsof -ti:$PROXY_PORT | xargs kill -9 >/dev/null 2>&1 || true
sleep 1

"$COWEN_BIN" daemon start --profile sidecar --foreground >"$COWEN_HOME/daemon.log" 2>&1 &
DAEMON_PID=$!
sleep 5

if grep -q "Local Proxy Server listening on" "$COWEN_HOME/daemon.log"; then
    echo -e "  ${GREEN}✓${NC} Daemon is running in foreground"
else
    cat "$COWEN_HOME/daemon.log"
    fail_suite "Daemon failed to start in foreground"
fi

echo -e "  Triggering AppTicket push..."
curl -s -X POST -H "appKey: AK_SA" "$MOCK_URL/auth/appTicket/resend" >/dev/null
sleep 2

echo -e "${BOLD}3. Trigger TEMP_AUTH_CODE Activation${NC}"
TEST_ORG_ID="ORG123"
TEMP_CODE="code_$TEST_ORG_ID"

# Push TEMP_AUTH_CODE via WebSocket broadcast
curl -s -X POST -H "Content-Type: application/json" \
    -d "{
        \"msgType\": \"TEMP_AUTH_CODE\",
        \"appKey\": \"AK_SA\",
        \"time\": \"$(date '+%Y-%m-%d %H:%M:%S')\",
        \"bizContent\": {
            \"tempAuthCode\": \"$TEMP_CODE\",
            \"state\": \"xyz\"
        }
    }" "$MOCK_URL/control/broadcast" >/dev/null

echo -e "  Waiting for exchange and archival..."
for i in {1..15}; do
    if grep -q "Enterprise permanent code successfully archived" "$COWEN_HOME/logs/sidecar_sys.log" || grep -q "Enterprise permanent code successfully archived" "$COWEN_HOME/daemon.log"; then
        echo -e "  ${GREEN}✓${NC} Permanent code archived for $TEST_ORG_ID"
        break
    fi
    sleep 1
done

if [ "$i" -eq 15 ]; then
    cat "$COWEN_HOME/daemon.log"
    cat "$COWEN_HOME/logs/sidecar_sys.log"
    fail_suite "Permanent code exchange timeout"
fi

echo -e "${BOLD}4. Verify Token Usage with Org ID${NC}"
echo -e "  Requesting API with x-org-id: $TEST_ORG_ID..."
RESP=$(curl -s -H "x-org-id: $TEST_ORG_ID" "http://127.0.0.1:$PROXY_PORT/v1/mock/secure")

assert_match "$RESP" "mock_at_oa2_permanent_code_" "Proxy used Org Access Token"
assert_match "$RESP" "verified" "API call successful"


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile sidecar 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 36 Passed! (StoreApp Activation & Org Token Usage)${NC}"
