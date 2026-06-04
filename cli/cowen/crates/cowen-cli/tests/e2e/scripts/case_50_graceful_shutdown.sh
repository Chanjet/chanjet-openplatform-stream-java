#!/bin/bash
# E2E Test: Phase 3 Graceful Shutdown (Case 50)
# Reference: cli/cowen/docs/WBS.md
#
# Architecture Note (v0.3.x IPC):
#   On Unix, `daemon start` dispatches workers to the standalone cowen-daemon
#   process via IPC. The drain/shutdown logs are emitted by bridge.rs inside
#   cowen-daemon and written to daemon.stdout.log.
#   To trigger a graceful shutdown, we use `daemon stop` which sends StopWorker
#   over IPC, causing the worker cancel_token to fire and the drain sequence
#   to execute inside cowen-daemon.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "case_50"
start_mock

# Configure delay in mock server to simulate slow webhook processing
curl -s -X POST "http://127.0.0.1:$MOCK_PORT/control/config" \
    -H "Content-Type: application/json" \
    -d '{"webhook_delay_ms": 3000}' >/dev/null

echo "--- Test 1: Setup Profile ---"
# Initialize self-built profile
"$COWEN_BIN" init \
    --profile main \
    --app-mode self-built \
    --app-key "test_key_shutdown" \
    --app-secret "test_secret_shutdown" \
    --encrypt-key 1234567890123456 \
    --certificate "test_cert" \
    --openapi-url "http://127.0.0.1:$MOCK_PORT" \
    --stream-url "ws://127.0.0.1:$MOCK_PORT" \
    --webhook-target "http://127.0.0.1:$MOCK_PORT/webhook_sink"

echo "--- Test 2: Start Daemon (IPC mode) ---"
"$COWEN_BIN" daemon start --profile main

echo "   Waiting for daemon to be ready..."
sleep 3

# Verify daemon is running via PID file
DAEMON_PID_FILE="$COWEN_HOME/master_daemon.pid"
if [ ! -f "$DAEMON_PID_FILE" ]; then
    fail_suite "Daemon PID file not found"
fi

DAEMON_PID=$(head -1 "$DAEMON_PID_FILE")
if ! kill -0 $DAEMON_PID 2>/dev/null; then
    fail_suite "Daemon process not running (PID: $DAEMON_PID)"
fi
echo "   Daemon running (PID: $DAEMON_PID)"

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

echo "--- Test 4: Send Stop Command and Verify Drain ---"
echo "   Sending 'daemon stop' to trigger graceful shutdown..."
"$COWEN_BIN" daemon stop --profile main

# Wait for daemon process to exit
for i in {1..20}; do
    if ! kill -0 $DAEMON_PID 2>/dev/null; then
        echo "   Daemon exited after ${i}x0.5s"
        break
    fi
    sleep 0.5
done

# Give a moment for log flush
sleep 1

echo "--- Test 5: Verify Log Contents ---"
DAEMON_LOG="$COWEN_HOME/logs/daemon.stdout.log"

if [ ! -f "$DAEMON_LOG" ]; then
    ls -la "$COWEN_HOME/logs/" 2>/dev/null
    fail_suite "daemon.stdout.log not found"
fi

# Check if "Shutdown signal received" or "Stopping worker (Draining)" was logged
if grep -q "Stopping worker (Draining)\|Shutdown signal received\|Waiting for active tasks to complete" "$DAEMON_LOG"; then
    echo "   ✓ Found drain/shutdown log"
else
    echo -e "${RED}FAILED: Log missing shutdown/drain indicators${NC}"
    tail -n 30 "$DAEMON_LOG"
    fail_suite "=== daemon.stdout.log (last 30 lines) ==="
fi

# Check if "All active tasks completed gracefully" was logged
if grep -q "All active tasks completed gracefully\|Timeout waiting for active tasks" "$DAEMON_LOG"; then
    echo "   ✓ Found graceful completion or timeout log"
else
    # If there were no active tasks at shutdown time, the drain log is skipped.
    # Check if the worker was stopped at all.
    if grep -q "Stopping worker\|Worker.*stopped" "$DAEMON_LOG"; then
        echo "   ✓ Worker stopped (no active tasks at shutdown time)"
    else
        echo -e "${RED}FAILED: Log missing drain completion indicator${NC}"
        tail -n 30 "$DAEMON_LOG"
        fail_suite "=== daemon.stdout.log (last 30 lines) ==="
    fi
fi

echo "--- Test 6: Verify Delivery at Sink ---"
# Ensure the webhook actually reached the sink despite the shutdown
SINK_CHECK=$(curl -s "http://127.0.0.1:$MOCK_PORT/control/webhooks")
if echo "$SINK_CHECK" | grep -q "value_for_shutdown_test"; then
    echo "   ✓ Webhook delivered successfully"
else
    fail_suite "Webhook was NOT delivered to the sink during shutdown"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo "--- Test 7: Verify Log Separation ---"
if [ -f "$COWEN_HOME/logs/daemon.stderr.log" ]; then
    if grep -q '"msg_type":"ping"\| INFO ' "$COWEN_HOME/logs/daemon.stderr.log"; then
        echo -e "${RED}FAILED: Found INFO logs or ping messages in daemon.stderr.log. Log separation is broken!${NC}"
        tail -n 30 "$COWEN_HOME/logs/daemon.stderr.log"
        fail_suite "Log separation broken"
    else
        echo "   ✓ daemon.stderr.log is clean of INFO/ping messages"
    fi
fi

echo -e "\n${GREEN}🎊 Case 50 Passed!${NC}"
cleanup_suite
exit 0
