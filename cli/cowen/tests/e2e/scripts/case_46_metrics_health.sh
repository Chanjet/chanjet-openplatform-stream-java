#!/bin/bash
# Test Case 46: Metrics and Health API
# Purpose: Verify that the cowen-monitor server correctly exposes /health and /metrics.

set -e
# Source common utilities
if [ -f "tests/e2e/scripts/common.sh" ]; then
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "46"
start_mock

echo -e "${BLUE}Starting Test Case 46: Metrics and Health API${NC}"

# 1. Initialize profile with Mock URL
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "CERT_SB" --encrypt-key "1234567890123456" --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" --webhook-target "http://127.0.0.1:8080" --no-telemetry --no-ai > /dev/null

# 2. Start daemon
MONITOR_PORT=$((16000 + RANDOM % 1000))
"$COWEN_BIN" config set --profile main monitor.port $MONITOR_PORT
"$COWEN_BIN" daemon start --profile main

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
    echo -e "${RED}[FAILED]${NC} Monitor server did not start. Got: $HEALTH_STATUS"
    exit 1
fi

# 4. Verify /metrics
echo "Checking /metrics..."
METRICS=$(curl -s http://127.0.0.1:$MONITOR_PORT/metrics)
if echo "$METRICS" | grep -q "cowen_active_connections"; then
    echo "Metrics endpoint returned active connections metric."
else
    echo -e "${RED}[FAILED]${NC} Metrics missing active_connections."
    exit 1
fi
echo "Metrics check passed."

# 5. Cleanup
cleanup_suite
echo -e "${GREEN}Test Case 46 PASSED!${NC}"
