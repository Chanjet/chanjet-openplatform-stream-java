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
sleep 2
"$COWEN_BIN" status --profile sidecar | grep -q "ACTIVE\|RUNNING"
assert_pass "Daemon is running"

echo -e "  Triggering AppTicket push..."
curl -s -X POST -H "appKey: AK_SA" "$MOCK_URL/auth/appTicket/resend" >/dev/null

echo -e "${BOLD}3. Token Validation${NC}"
# StoreApp acquires App Access Token dynamically via Daemon, not via manual login.
# We wait for the daemon to process the push and acquire it.
for i in {1..20}; do
    T=$(extract_token sidecar)
    if [ -n "$T" ] && [[ "$T" == mock_at_sa* ]]; then
        echo -e "  ${GREEN}✓${NC} Token acquired: $T"
        break
    fi
    sleep 1
done

if [ "$i" -eq 20 ]; then
    echo -e "  ${RED}✗${NC} Token acquisition failed or still waiting"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 02 Passed!${NC}"
