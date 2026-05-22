#!/bin/bash
set -e
# Case 22: Manual DLQ Intervention (Dead Letter Queue)
# Verifies:
#   1. Failing webhooks are stored in DLQ.
#   2. 'cowen dlq list' displays the failed messages.
#   3. 'cowen dlq retry <ID>' successfully forwards the message after sink recovery.
#   4. Retried messages are removed from DLQ.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_22"
start_mock

PROF="dlq_manual"

# Initialize with a sink that we can control
"$COWEN_BIN" init --profile "$PROF" --app-mode self-built \
    --app-key AK_DLQ --app-secret AS_DLQ --certificate CERT_DLQ --encrypt-key 1234567890123456 \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port $PROXY_PORT >/dev/null

# Start Daemon
"$COWEN_BIN" daemon start --profile "$PROF" >/dev/null 2>&1
sleep 3

# 2. Inject Failure
echo -e "${BOLD}2. Simulating Sink Failure (500 Error)${NC}"
# Configure mock sink to return 500
curl -s -X POST "$MOCK_URL/control/config" -d '{"webhook_sink_status": 500}' > /dev/null

# Broadcast a message
MSG_ID="MSG_DLQ_$(date +%s)"
curl -s -X POST "$MOCK_URL/control/broadcast" -d "{
    \"msgType\": \"DATA_PUSH\",
    \"appKey\": \"AK_DLQ\",
    \"msgId\": \"$MSG_ID\",
    \"payload\": {\"data\": \"critical_business_data\"}
}" > /dev/null

echo "   Waiting for daemon to process and fail (putting msg into DLQ)..."
# Wait enough time for initial failure and DLQ insertion (Cowen retries internally, we just need it in DB)
sleep 5

# Check if it's in DLQ
DLQ_OUT=$("$COWEN_BIN" dlq list --profile "$PROF" --format json)
if echo "$DLQ_OUT" | grep -q "$MSG_ID"; then
    echo -e "   ${GREEN}✓${NC} Message successfully captured in DLQ"
else
    echo -e "   ${RED}[FAILED]${NC} Message not found in DLQ"
    echo "$DLQ_OUT"
    exit 1
fi

# Extract the internal DB ID of the DLQ item
# The JSON output is an array of objects. We need the 'id' field of the matched message.
DLQ_ID=$(echo "$DLQ_OUT" | python3 -c "import sys,json; data=json.loads(sys.stdin.read()); print(next(item['id'] for item in data if '$MSG_ID' in str(item)))" 2>/dev/null)

if [ -z "$DLQ_ID" ]; then
    echo -e "   ${RED}[FAILED]${NC} Could not extract DLQ ID"
    exit 1
fi
echo "   Extracted DLQ ID: $DLQ_ID"

# 3. Recover Sink and Manual Retry
echo -e "${BOLD}3. Recovering Sink and Executing Manual Retry${NC}"
curl -s -X POST "$MOCK_URL/control/config" -d '{"webhook_sink_status": 200}' > /dev/null
curl -s -X POST "$MOCK_URL/control/clear_webhooks" > /dev/null

# Execute manual retry
"$COWEN_BIN" dlq retry "$DLQ_ID" --profile "$PROF" > /dev/null
sleep 2

# 4. Verify Forwarding and Deletion
echo -e "${BOLD}4. Verifying Successful Forwarding and DLQ Cleanup${NC}"
SINK_CHECK=$(curl -s "$MOCK_URL/control/webhooks")
if echo "$SINK_CHECK" | grep -q "$MSG_ID"; then
    echo -e "   ${GREEN}✓${NC} Message successfully delivered to Sink after manual retry"
else
    echo -e "   ${RED}[FAILED]${NC} Message not found at Sink after retry"
    exit 1
fi

# Verify DLQ is empty or at least our specific ID is gone
DLQ_OUT_POST=$("$COWEN_BIN" dlq list --profile "$PROF" --format json)
if echo "$DLQ_OUT_POST" | grep -q "$DLQ_ID"; then
    echo -e "   ${RED}[FAILED]${NC} The specific DLQ entry ($DLQ_ID) still exists after successful retry"
    exit 1
else
    echo -e "   ${GREEN}✓${NC} Specific DLQ entry removed"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 22 Passed!${NC}"
