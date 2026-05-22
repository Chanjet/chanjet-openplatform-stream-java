#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "store_app"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile sidecar \
    --app-mode store-app \
    --app-key AK_SA \
    --app-secret AS_SA \
    --encrypt-key 1234567890123456 \
    --webhook-target "http://127.0.0.1:8080/cb" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Daemon Startup${NC}"
"$COWEN_BIN" daemon start --profile sidecar >/dev/null
assert_pass "Daemon start command sent"
wait_for_daemon sidecar 10
assert_pass "Daemon is running"

echo -e "  Triggering AppTicket push..."
curl -s -X POST -H "appKey: AK_SA" "$MOCK_URL/auth/appTicket/resend" >/dev/null

echo -e "${BOLD}3. Token Validation${NC}"
T=$(wait_for_token sidecar mock_at_sa 20)
if [ -z "$T" ]; then
    echo -e "  ${RED}✗${NC} Token acquisition failed or still waiting"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Token acquired: $T"

# 4. Mandatory Sanitization Check
echo -e "${BOLD}4. Mandatory Sanitization Check${NC}"
CONFIG_OUT=$("$COWEN_BIN" config --profile sidecar 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 02 Passed!${NC}"

