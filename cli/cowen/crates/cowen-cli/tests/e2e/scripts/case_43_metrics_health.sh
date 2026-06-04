#!/bin/bash
# Test Case 43: Metrics and Health API
# Purpose: Verify that the cowen-monitor server correctly exposes /health and /metrics.

set -e
# Source common utilities
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_43"
start_mock

echo -e "${BLUE}Starting Test Case 43: Metrics and Health API${NC}"

# 1. Initialize profile with Mock URL
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "CERT_SB" --encrypt-key "1234567890123456" --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" --webhook-target "http://127.0.0.1:8080" --no-telemetry > /dev/null

# 2. Start daemon
"$COWEN_BIN" daemon start --profile main

echo "Waiting for daemon to write PID file..."
for i in {1..20}; do
    if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
        if grep -q "MONITOR_PORT" "$COWEN_HOME/master_daemon.pid"; then
            break
        fi
    fi
    sleep 0.5
done

MONITOR_PORT=$(grep "MONITOR_PORT" "$COWEN_HOME/master_daemon.pid" 2>/dev/null | cut -d= -f2)

if [ -z "$MONITOR_PORT" ]; then
    cat "$COWEN_HOME/master_daemon.pid" 2>/dev/null || echo "PID file missing"
    fail_suite "Could not extract MONITOR_PORT from master_daemon.pid"
fi

echo "Extracted monitor server port: $MONITOR_PORT"
echo "Waiting for monitor server on port $MONITOR_PORT..."
for i in {1..15}; do
    if curl -s http://127.0.0.1:$MONITOR_PORT/health | grep -q "UP"; then
        echo "Monitor server ready."
        break
    fi
    sleep 2
done

# 3. Verify /health
echo "Checking /health on port $MONITOR_PORT..."
HEALTH_STATUS=$(curl -s http://127.0.0.1:$MONITOR_PORT/health || echo "DOWN")
if [ "$HEALTH_STATUS" == "UP" ]; then
    echo "Health check passed."
else
    fail_suite "Monitor server did not start. Got: $HEALTH_STATUS"
fi

# 4. Verify /metrics
echo "Checking /metrics..."
METRICS=$(curl -s http://127.0.0.1:$MONITOR_PORT/metrics)
if echo "$METRICS" | grep -q "cowen_active_connections"; then
    echo "Metrics endpoint returned active connections metric."
else
    fail_suite "Metrics missing active_connections."
fi
echo "Metrics check passed."

# 5. Cleanup
cleanup_suite
echo -e "${GREEN}Test Case 43 PASSED!${NC}"
