#!/bin/bash
# Test Case 46: Metrics and Health API
# Purpose: Verify that the cowen-monitor server correctly exposes /health and /metrics.

set -e
NC='\033[0m'
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'

echo -e "${BLUE}Starting Test Case 46: Metrics and Health API${NC}"

# 1. Setup environment
TEST_ID=$RANDOM
export COWEN_HOME="$(pwd)/target/cowen_tests/case_46_$TEST_ID"
mkdir -p "$COWEN_HOME"
TEST_PROFILE="monitor_test_$TEST_ID"
MONITOR_PORT=$((16000 + TEST_ID % 500))

# 2. Initialize profile
$COWEN_BIN init --profile "$TEST_PROFILE" --app-mode self-built --app-key "k" --app-secret "s" --certificate "c" --encrypt-key "e" --stream-url "http://localhost:8080" > /dev/null
# 3. Start daemon with explicit monitor port
$COWEN_BIN config set --profile "$TEST_PROFILE" monitor.port $MONITOR_PORT
$COWEN_BIN daemon start --profile "$TEST_PROFILE"

echo "Waiting for monitor server on port $MONITOR_PORT..."
for i in {1..10}; do
    if curl -s http://127.0.0.1:$MONITOR_PORT/health > /dev/null; then
        echo "Monitor server ready."
        break
    fi
    sleep 2
done

# 4. Verify /health
echo "Checking /health on port $MONITOR_PORT..."
for i in {1..20}; do
    HEALTH_STATUS=$(curl -s http://127.0.0.1:$MONITOR_PORT/health || echo "DOWN")
    if [ "$HEALTH_STATUS" == "UP" ]; then
        echo "Health check passed."
        break
    fi
    echo "Health check failed (got $HEALTH_STATUS), retrying..."
    sleep 2
done
if [ "$HEALTH_STATUS" != "UP" ]; then
    echo -e "${RED}[FAILED]${NC} Monitor server did not start."
    exit 1
fi

# 5. Verify /metrics
echo "Checking /metrics..."
METRICS=$(curl -s http://127.0.0.1:$MONITOR_PORT/metrics)
if echo "$METRICS" | grep -q "cowen_active_connections"; then
    echo "Metrics endpoint returned active connections metric."
else
    echo -e "${RED}[FAILED]${NC} Metrics missing active_connections."
    exit 1
fi
echo "Metrics check passed."

# 6. Cleanup
$COWEN_BIN daemon stop --profile "$TEST_PROFILE"
echo -e "${GREEN}Test Case 46 PASSED!${NC}"
rm -rf "$COWEN_HOME"
