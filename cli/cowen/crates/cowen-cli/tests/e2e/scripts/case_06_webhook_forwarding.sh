#!/bin/bash
set -e
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)

setup_workspace "case_06"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization with Webhook Target${NC}"
# Point webhook-target to our mock sink
SINK_URL="http://127.0.0.1:9299/webhook_sink"
"$COWEN_BIN" init --profile fwd --app-mode self-built \
    --app-key AK_FWD --app-secret AS_FWD --encrypt-key 1234567890123456 --certificate CERT_FWD \
    --webhook-target "$SINK_URL" \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Daemon${NC}"
"$COWEN_BIN" daemon start --profile fwd >/dev/null
wait_for_daemon fwd 10
assert_pass "Daemon is running and connected to WS"

echo -e "${BOLD}3. Trigger External Broadcast${NC}"
# Use control API of mock server to send a DATA_PUSH message
PAYLOAD='{"orderId":"ORD123","amount":"99.9"}'
curl -s -X POST -H "Content-Type: application/json" \
     -d "{\"msg_type\":\"DATA_PUSH\",\"payload\":$PAYLOAD}" \
     http://127.0.0.1:9299/control/broadcast >/dev/null
assert_pass "Broadcast triggered from platform"

echo -e "${BOLD}4. Verify Forwarding at Sink${NC}"
echo "   Waiting for daemon to process and forward..."
sleep 3
SINK_CHECK=$(curl -s http://127.0.0.1:9299/control/webhooks)
echo "   Sink Status: $SINK_CHECK"
if echo "$SINK_CHECK" | grep -q "DATA_PUSH"; then
    echo -e "  ${GREEN}✓${NC} Webhook successfully forwarded to sink"
else
    fail_suite "Webhook NOT found at sink"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile fwd 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 06 Passed!${NC}"
