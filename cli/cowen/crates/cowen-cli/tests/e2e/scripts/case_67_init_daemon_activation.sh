#!/bin/bash
# Test Case 67: Init Daemon Activation
# Verifies that when a master daemon is running, initializing a new profile 
# automatically activates its daemon capabilities (bridge connection, token sync).

set -e

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"



fi

PROXY_PORT=$(get_unused_port)

setup_workspace "case_67"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Start Master Process with an initial profile${NC}"
# Initialize the first profile to start the master daemon
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_MAIN \
    --app-secret AS_MAIN \
    --certificate CERT_MAIN \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null

"$COWEN_BIN" daemon start --all >/dev/null
wait_for_daemon main 10
assert_pass "Master Daemon is running"

echo -e "${BOLD}2. Init new profile${NC}"
PROXY_PORT_NEW=$(get_unused_port)
"$COWEN_BIN" init --profile new_profile \
    --app-mode self-built \
    --app-key TEST_NEW \
    --app-secret SECRET_NEW \
    --encrypt-key 1234567890123456 \
    --certificate CERT_NEW \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT_NEW >/dev/null

echo -e "${BOLD}3. Check status of new_profile (Expect: Connected)${NC}"
# We wait a short moment for it to theoretically connect
sleep 3

STATUS_OUT=$("$COWEN_BIN" status --profile new_profile)
echo "$STATUS_OUT"

if echo "$STATUS_OUT" | grep -q "Bridge Connection" && echo "$STATUS_OUT" | grep -q -i "Connected"; then
    echo -e "  ${GREEN}✓${NC} Bridge Connection is Connected"
else
    echo -e "  ${RED}✗${NC} Bridge Connection is NOT Connected"
    fail_suite "new_profile did not automatically activate daemon connection"
fi

if echo "$STATUS_OUT" | grep -q "AccessToken" && ! echo "$STATUS_OUT" | grep -q "Not initialized"; then
    echo -e "  ${GREEN}✓${NC} AccessToken is initialized"
else
    echo -e "  ${RED}✗${NC} AccessToken is Not initialized"
    fail_suite "new_profile did not automatically fetch access token"
fi

pass_suite
