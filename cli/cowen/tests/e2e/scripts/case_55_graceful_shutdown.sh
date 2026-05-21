#!/bin/bash
# E2E Test: Phase 3 Graceful Shutdown (Case 55)
# Reference: cli/cowen/docs/WBS.md

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "graceful_shutdown"
start_mock

# Configure delay in mock server to simulate slow webhook processing
curl -s -X POST "http://127.0.0.1:$MOCK_PORT/control/config" \
    -H "Content-Type: application/json" \
    -d '{"webhook_delay_ms": 3000}' >/dev/null

echo "--- Test 1: Setup Profile ---"
# Initialize self-built profile
"$COWEN_BIN" init \
    --app-mode self-built \
    --app-key "test_key_shutdown" \
    --app-secret "test_secret_shutdown" \
    --encrypt-key 1234567890123456 \
    --certificate "test_cert" \
    --openapi-url "http://127.0.0.1:$MOCK_PORT" \
    --stream-url "ws://127.0.0.1:$MOCK_PORT" \
    --webhook-target "http://127.0.0.1:$MOCK_PORT/webhook_sink"

echo "--- Test 2: Start Daemon ---"
# We start it in the background manually to capture its PID and stdout
"$COWEN_BIN" daemon start --foreground > "$TEST_BASE/daemon_foreground.log" 2>&1 &
DAEMON_PID=$!

echo "   Waiting for daemon to be ready..."
sleep 3

# Verify it is running
if ! kill -0 $DAEMON_PID 2>/dev/null; then
    echo -e "${RED}FAILED: Daemon failed to start${NC}"
    cat "$TEST_BASE/daemon_foreground.log"
    exit 1
fi

echo "--- Test 3: Trigger High-Latency Forwarding ---"
# Trigger a push from the platform
PAYLOAD="{\"some_data\":\"value_for_shutdown_test\"}"
curl -s -X POST \
     -H "Content-Type: application/json" \
     -H "appKey: test_key_shutdown" \
     -d "{\"msg_type\":\"DATA_PUSH\",\"payload\":$PAYLOAD}" \
     http://127.0.0.1:$MOCK_PORT/control/broadcast >/dev/null

echo "   Event triggered. Wait 1 second to let daemon start processing..."
sleep 1

echo "--- Test 4: Send SIGTERM and Verify Drain ---"
echo "   Sending SIGTERM to Daemon PID $DAEMON_PID..."
kill -15 $DAEMON_PID

# Wait for daemon to exit. 
# We allow 0 (normal exit) and 143 (SIGTERM reported by shell)
wait $DAEMON_PID || true
# Since we are not in 'set -e', we manually check the state if needed, 
# but the log check is more definitive.

echo "--- Test 5: Verify Log Contents ---"
# Check if "Waiting for active tasks to complete" was logged
if grep -q "Waiting for active tasks to complete" "$TEST_BASE/daemon_foreground.log"; then
    echo "   ✓ Found drain log"
else
    echo -e "${RED}FAILED: Log missing 'Waiting for active tasks to complete'${NC}"
    cat "$TEST_BASE/daemon_foreground.log"
    exit 1
fi

# Check if "All active tasks completed gracefully" was logged
if grep -q "All active tasks completed gracefully" "$TEST_BASE/daemon_foreground.log"; then
    echo "   ✓ Found graceful completion log"
else
    echo -e "${RED}FAILED: Log missing 'All active tasks completed gracefully'${NC}"
    cat "$TEST_BASE/daemon_foreground.log"
    exit 1
fi

echo "--- Test 6: Verify Delivery at Sink ---"
# Ensure the webhook actually reached the sink despite the shutdown
SINK_CHECK=$(curl -s "http://127.0.0.1:$MOCK_PORT/control/webhooks")
if echo "$SINK_CHECK" | grep -q "value_for_shutdown_test"; then
    echo "   ✓ Webhook delivered successfully"
else
    echo -e "${RED}FAILED: Webhook was NOT delivered to the sink during shutdown${NC}"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 55 Passed!${NC}"
cleanup_suite
exit 0
