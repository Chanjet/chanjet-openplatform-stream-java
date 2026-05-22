#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "self_built"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Daemon Startup${NC}"
"$COWEN_BIN" daemon start --profile main >/dev/null
assert_pass "Daemon start command sent"
wait_for_daemon main 10
assert_pass "Daemon is running"

echo -e "${BOLD}3. Token Acquisition (WS Push)${NC}"
# Login triggers the proactive push, but we need to wait for the background daemon to process it
"$COWEN_BIN" auth login --profile main --force >/dev/null
T=$(wait_for_token main mock_at_sb 10)
if [ -z "$T" ]; then
    echo -e "  ${RED}✗${NC} Token acquisition failed"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Token acquired: $T"

# 4. Mandatory Sanitization Check
echo -e "${BOLD}4. Mandatory Sanitization Check${NC}"
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 01 Passed!${NC}"

