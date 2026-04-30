#!/bin/bash
source tests/common.sh

setup_workspace "dlq_test"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization with BROKEN Webhook Target${NC}"
# Use a non-existent port to force connection failure
BAD_SINK="http://127.0.0.1:9999/broken"
"$COWEN_BIN" init --profile dlq_prof --app-mode self-built \
    --app-key AK_DLQ --app-secret AS_DLQ --encrypt-key 1234567890123456 --certificate CERT_DLQ \
    --webhook-target "$BAD_SINK" \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port 9909 >/dev/null
assert_pass "Profile initialized"

"$COWEN_BIN" auth login --profile dlq_prof --force >/dev/null
assert_pass "Initial token acquired"

echo -e "${BOLD}2. Start Daemon${NC}"
"$COWEN_BIN" daemon start --profile dlq_prof >/dev/null
echo -n "  Waiting for daemon bridge connection..."
for i in {1..20}; do
    if grep -q "Bridge connection established" "$COWEN_HOME/logs/dlq_prof_sys.log" 2>/dev/null; then
        echo -e " ${GREEN}[CONNECTED]${NC}"
        break
    fi
    echo -n "."
    sleep 1
done
assert_pass "Daemon is running and connected"

echo -e "${BOLD}3. Trigger Broadcast (Will Fail Forwarding)${NC}"
curl -s -X POST -H "Content-Type: application/json" \
     -d '{"msg_type":"DLQ_TRIGGER","payload":{"test":"fail"}}' \
     http://127.0.0.1:9299/control/broadcast >/dev/null
assert_pass "Broadcast triggered"

echo -e "${BOLD}4. Verify DLQ Recording${NC}"
echo "   Waiting for retries and DLQ storage..."
sleep 10
DLQ_OUT=$("$COWEN_BIN" dlq list --profile dlq_prof)
echo "   DLQ List: $DLQ_OUT"
if echo "$DLQ_OUT" | grep -q "DLQ_TRIGGER"; then
    echo -e "  ${GREEN}✓${NC} Message successfully recorded in DLQ"
else
    echo -e "  ${RED}✗${NC} Message NOT found in DLQ"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 09 Passed!${NC}"
