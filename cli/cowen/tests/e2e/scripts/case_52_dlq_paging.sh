#!/bin/bash
# E2E Test: Phase 5 DLQ Paging & Precise Retry (Case 52)
# Reference: cli/cowen/docs/WBS.md

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "dlq_paging"
start_mock

echo "--- Test 1: Setup Profile with Invalid Webhook (to trigger DLQ) ---"
# Use a non-existent port to force connection failure
"$COWEN_BIN" init \
    --app-mode self-built \
    --app-key "test_key_dlq" \
    --app-secret "test_secret_dlq" \
    --encrypt-key 1234567890123456 \
    --certificate "test_cert" \
    --openapi-url "http://127.0.0.1:$MOCK_PORT" \
    --stream-url "ws://127.0.0.1:$MOCK_PORT" \
    --webhook-target "http://127.0.0.1:1" # Invalid port

echo "--- Test 2: Start Daemon and Inject 25 Failed Messages ---"
"$COWEN_BIN" daemon start --foreground > "$TEST_BASE/daemon.log" 2>&1 &
DAEMON_PID=$!

echo "   Waiting for daemon to be ready..."
sleep 3

for i in {1..25}; do
    PAYLOAD="{\"msg_id\":\"msg_$i\",\"data\":\"value_$i\"}"
    curl -s -X POST \
         -H "Content-Type: application/json" \
         -H "appKey: test_key_dlq" \
         -d "{\"msg_type\":\"DATA_PUSH\",\"payload\":$PAYLOAD}" \
         http://127.0.0.1:$MOCK_PORT/control/broadcast >/dev/null
    sleep 0.1
done

echo "   Wait for messages to hit DLQ..."
sleep 5
kill $DAEMON_PID

echo "--- Test 3: Verify DLQ Paging (Default 20 items) ---"
LIST_OUTPUT=$("$COWEN_BIN" dlq list)
COUNT=$(echo "$LIST_OUTPUT" | grep -c "ID:")
echo "   Items in Page 1: $COUNT"
if [ "$COUNT" -ne 20 ]; then
    echo -e "${RED}FAILED: Expected 20 items in default list, found $COUNT${NC}"
    echo "$LIST_OUTPUT"
    exit 1
fi
echo "   ✓ Page 1 paging works"

echo "--- Test 4: Verify DLQ Page 2 ---"
LIST_OUTPUT_P2=$("$COWEN_BIN" dlq list --page 2)
COUNT_P2=$(echo "$LIST_OUTPUT_P2" | grep -c "ID:")
echo "   Items in Page 2: $COUNT_P2"
if [ "$COUNT_P2" -lt 5 ]; then
    echo -e "${RED}FAILED: Expected at least 5 items in page 2, found $COUNT_P2${NC}"
    echo "$LIST_OUTPUT_P2"
    exit 1
fi
echo "   ✓ Page 2 paging works"

echo "--- Test 5: Verify Precise Retry ---"
# Get the first ID from page 2
FIRST_ID_P2=$(echo "$LIST_OUTPUT_P2" | grep "ID:" | head -n 1 | sed -n 's/.*ID: \([0-9]*\).*/\1/p')
echo "   Retrying ID: $FIRST_ID_P2"

# Temporarily fix webhook target to let it succeed
# Actually we can just run retry and see if it attempts to delete it
# We update config to a valid sink
"$COWEN_BIN" config set webhook_target "http://127.0.0.1:$MOCK_PORT/webhook_sink"

"$COWEN_BIN" dlq retry "$FIRST_ID_P2"
if [ $? -ne 0 ]; then
    echo -e "${RED}FAILED: dlq retry command failed${NC}"
    exit 1
fi

# Verify it's gone from DLQ
LIST_OUTPUT_P2_AFTER=$("$COWEN_BIN" dlq list --page 2)
if echo "$LIST_OUTPUT_P2_AFTER" | grep -q "ID: ${FIRST_ID_P2}\b"; then
    echo -e "${RED}FAILED: Item $FIRST_ID_P2 still in DLQ after retry${NC}"
    exit 1
fi
echo "   ✓ Precise retry and deletion successful"

echo -e "\n${GREEN}🎊 Case 52 Passed!${NC}"
cleanup_suite
exit 0
