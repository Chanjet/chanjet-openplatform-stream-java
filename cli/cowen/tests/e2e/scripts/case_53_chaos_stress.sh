#!/bin/bash
# E2E Test: Chaos Stress & Graceful Shutdown (Case 53)
# This test verifies that the daemon remains consistent and releases resources
# even when killed under heavy concurrent load.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "chaos_stress"
start_mock

# Configure moderate delay in mock to keep connections alive longer
curl -s -X POST "http://127.0.0.1:$MOCK_PORT/control/config" \
    -H "Content-Type: application/json" \
    -d '{"webhook_delay_ms": 1000}' >/dev/null

echo "--- Test 1: Setup Profile ---"
"$COWEN_BIN" init \
    --app-mode self-built \
    --app-key "chaos_stress_key" \
    --app-secret "chaos_stress_secret" \
    --encrypt-key 12345678901234567890123456789012 \
    --certificate "chaos_cert" \
    --openapi-url "http://127.0.0.1:$MOCK_PORT" \
    --stream-url "ws://127.0.0.1:$MOCK_PORT" \
    --webhook-target "http://127.0.0.1:$MOCK_PORT/webhook_sink"

echo "--- Test 2: Start Daemon ---"
DAEMON_LOG="$TEST_BASE/daemon_chaos.log"
"$COWEN_BIN" daemon start --foreground > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!

echo "   Waiting for daemon to be ready..."
sleep 3

if ! kill -0 $DAEMON_PID 2>/dev/null; then
    echo -e "${RED}FAILED: Daemon failed to start${NC}"
    cat "$DAEMON_LOG"
    exit 1
fi

echo "--- Test 3: Inject Heavy Concurrent Load ---"
# Fire 40 concurrent messages to saturate workers
echo "   Broadcasting 40 DATA_PUSH events..."
for i in {1..40}; do
    curl -s -X POST \
         -H "Content-Type: application/json" \
         -H "appKey: chaos_stress_key" \
         -d "{\"msg_type\":\"DATA_PUSH\",\"payload\":{\"index\":$i}}" \
         "http://127.0.0.1:$MOCK_PORT/control/broadcast" >/dev/null &
done

# Small sleep to let the messages hit the daemon and trigger forwarders
sleep 1.5

echo "--- Test 4: Send SIGTERM at Peak Load ---"
echo "   Killing Daemon (PID: $DAEMON_PID) with SIGTERM..."
kill -15 $DAEMON_PID

# Wait for exit with timeout
WAIT_TIMEOUT=15
START_TIME=$(date +%s)
while kill -0 $DAEMON_PID 2>/dev/null; do
    ELAPSED=$(( $(date +%s) - START_TIME ))
    if [ $ELAPSED -ge $WAIT_TIMEOUT ]; then
        echo -e "${RED}FAILED: Daemon failed to exit within ${WAIT_TIMEOUT}s${NC}"
        kill -9 $DAEMON_PID 2>/dev/null
        exit 1
    fi
    sleep 0.5
done
echo "   Daemon exited gracefully."

echo "--- Test 5: Verify Integrity & Schema ---"
# Use cowen doctor to ensure storage is not corrupted/locked and schema is valid
if ! "$COWEN_BIN" doctor --fix > /dev/null 2>&1; then
    echo -e "${RED}FAILED: Cowen doctor reported errors after chaos shutdown${NC}"
    "$COWEN_BIN" doctor --verbose
    exit 1
fi
echo "   ✓ Storage and Schema integrity verified"

echo "--- Test 6: Verify Log Success Markers ---"
if grep -qE "All active tasks completed gracefully|No active tasks, proceeding with shutdown" "$DAEMON_LOG"; then
    echo "   ✓ Found graceful completion marker in logs"
else
    # In extreme stress, some tasks might be killed by the 10s hard timeout, 
    # but the drain attempt must have been logged.
    if grep -q "Waiting for active tasks to complete" "$DAEMON_LOG"; then
        echo "   ℹ️ Found drain attempt marker (tasks may have timed out but protocol followed)"
    else
        echo -e "${RED}FAILED: Log missing shutdown protocol markers${NC}"
        tail -n 20 "$DAEMON_LOG"
        exit 1
    fi
fi

echo -e "\n${GREEN}🎊 Case 53 Passed!${NC}"
cleanup_suite
exit 0
