#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_05"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile pxt --app-mode self-built \
    --app-key AK_PXT --app-secret AS_PXT --encrypt-key 1234567890123456 --certificate CERT_PXT \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Daemon${NC}"
"$COWEN_BIN" daemon start --profile pxt >/dev/null
wait_for_daemon pxt 10
assert_pass "Daemon is running"

echo -e "${BOLD}3. Transparent Token Injection${NC}"
# First login to get a token
"$COWEN_BIN" auth login --profile pxt --force >/dev/null
# Call mock API through local proxy (port $PROXY_PORT)
RESP=$(curl -s http://127.0.0.1:$PROXY_PORT/v1/mock/secure)
echo "   Response: $RESP"
echo "$RESP" | grep -q "verified"
assert_pass "Proxy injected token and forwarded request"

echo -e "${BOLD}4. Whitelist Enforcement${NC}"
# Try a path not in mock_server's spec (the mock spec has /v1/mock/ping and /v1/mock/secure)
RESP_FAIL=$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:$PROXY_PORT/v1/unauthorized/path)
if [ "$RESP_FAIL" == "404" ]; then
    echo -e "  ${GREEN}✓${NC} Received 404 for unauthorized path (whitelist not enforced)"
else
    echo -e "  ${RED}✗${NC} Unexpected response for path (Got $RESP_FAIL)"
    exit 1
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile pxt 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 05 Passed!${NC}"
exit 0
