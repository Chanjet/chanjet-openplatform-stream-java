#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
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
    --proxy-port 9091 >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Daemon Startup${NC}"
"$COWEN_BIN" daemon start --profile main >/dev/null
echo "   [WAIT] Waiting for WS handshake..."
sleep 5
"$COWEN_BIN" status --profile main | grep -q "ACTIVE\|RUNNING"
assert_pass "Daemon is running"

echo -e "${BOLD}3. Token Acquisition (WS Push)${NC}"
# Login triggers the proactive push, but we need to wait for the background daemon to process it
"$COWEN_BIN" auth login --profile main --force >/dev/null
for i in {1..10}; do
    T=$(extract_token main)
    if [ -n "$T" ] && [[ "$T" == mock_at_sb* ]]; then
        echo -e "  ${GREEN}✓${NC} Token acquired: $T"
        break
    fi
    sleep 1
done

if [ "$i" -eq 10 ]; then
    echo -e "  ${RED}✗${NC} Token acquisition failed"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 01 Passed!${NC}"
