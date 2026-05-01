#!/bin/bash
# Case 11: WebSocket Reconnection Resilience
# Simulates network drop or service rolling restart by force-closing WS connections on the mock server.

source tests/common.sh

setup_workspace "reconnect_test"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
SINK_URL="http://127.0.0.1:9299/webhook_sink"
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_RECONNECT \
    --app-secret AS_RECONNECT \
    --encrypt-key 1234567890123456 \
    --certificate CERT_RECONNECT \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$SINK_URL" \
    --proxy-port 9091 >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Daemon & Establish Connection${NC}"
"$COWEN_BIN" daemon start --profile main >/dev/null
assert_pass "Daemon started"

# Wait for bridge connection
echo -n "   Waiting for bridge connection..."
for i in {1..10}; do
    if "$COWEN_BIN" status | grep -q "Connected"; then
        echo -e " ${GREEN}[CONNECTED]${NC}"
        break
    fi
    if [ $i -eq 10 ]; then
        echo -e " ${RED}[TIMEOUT]${NC}"
        exit 1
    fi
    sleep 1
    echo -n "."
done

echo -e "${BOLD}3. Simulate Service Rolling Update (Force Close WS)${NC}"
# Use the new /control/kill_connections endpoint
curl -s -X POST "$MOCK_URL/control/kill_connections" >/dev/null
echo -e "   ${YELLOW}⚡ WS connection killed on server side${NC}"

# Verify it's disconnected
sleep 0.5
STATUS=$("$COWEN_BIN" status)
if echo "$STATUS" | grep -q "Disconnected" || echo "$STATUS" | grep -q "Reconnecting"; then
    echo -e "   ${GREEN}✓${NC} Daemon detected disconnection"
else
    echo -e "   ${RED}✗${NC} Daemon still thinks it's connected (or status didn't update)"
    echo "$STATUS"
    exit 1
fi

echo -e "${BOLD}4. Verify Automatic Reconnection${NC}"
echo -n "   Waiting for automatic reconnection..."
for i in {1..20}; do
    if "$COWEN_BIN" status | grep -q "Connected"; then
        echo -e " ${GREEN}[RECONNECTED]${NC}"
        break
    fi
    if [ $i -eq 20 ]; then
        echo -e " ${RED}[TIMEOUT]${NC}"
        "$COWEN_BIN" status
        exit 1
    fi
    sleep 1
    echo -n "."
done

echo -e "${BOLD}5. Functional Check after Reconnection${NC}"
# Trigger a broadcast to see if it's still receiving
curl -s -X POST "$MOCK_URL/control/broadcast" \
    -H "Content-Type: application/json" \
    -d '{"msg_type": "RECONNECT_TEST", "payload": {"status": "ok_after_retry"}}' >/dev/null

sleep 2
# Check if message reached the webhook sink via daemon
MESSAGES=$(curl -s "$MOCK_URL/control/webhooks")
if echo "$MESSAGES" | grep -q "RECONNECT_TEST"; then
    echo -e "   ${GREEN}✓${NC} Received message after reconnection"
else
    echo -e "   ${RED}✗${NC} Failed to receive message after reconnection"
    echo "$MESSAGES"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 11 Passed!${NC}"
cleanup_suite
