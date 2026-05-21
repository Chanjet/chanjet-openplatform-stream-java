#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "lifecycle"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile life --app-mode self-built \
    --app-key AK_LIFE --app-secret AS_LIFE --encrypt-key 1234567890123456 --certificate CERT_LIFE \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Daemon${NC}"
"$COWEN_BIN" daemon start --profile life >/dev/null
sleep 2
assert_pass "Daemon is running"

echo -e "${BOLD}3. Initial Token Acquisition${NC}"
"$COWEN_BIN" auth login --profile life --force >/dev/null
T1=$(extract_token life)
echo "   Token 1: $T1"
assert_pass "First token acquired"

echo -e "${BOLD}4. Manual Token Invalidation (simulate expiration)${NC}"
# Inject an expired timestamp into the InnerDB (cowen_app_token for Self-Built mode)
sqlite3 "$COWEN_HOME/cowen.db" "UPDATE cowen_app_token SET token_value = 'expired-token', expires_at = '2000-01-01T00:00:00Z' WHERE app_key = 'AK_LIFE';"
assert_pass "Token marked as expired in store"

echo -e "${BOLD}5. Transparent Refresh via Proxy${NC}"
# The proxy should detect expiration and refresh before forwarding
RESP=$(curl -s http://127.0.0.1:$PROXY_PORT/v1/mock/secure)
T2=$(extract_token life)
echo "   Token 2: $T2"

if [ "$T1" != "$T2" ] && [[ "$T2" == mock_at_sb* ]]; then
    echo -e "  ${GREEN}✓${NC} Token automatically rotated by proxy"
else
    echo -e "  ${RED}✗${NC} Token rotation failed"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 07 Passed!${NC}"
