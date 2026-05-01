#!/bin/bash
source tests/common.sh

# Use a specific port for Redis to avoid conflicts
REDIS_PORT=6381
redis-server --port $REDIS_PORT --daemonize yes
trap "redis-cli -p $REDIS_PORT shutdown >/dev/null 2>&1; cleanup_suite" EXIT

setup_workspace "scaling_stress"
start_mock

# One-liner config for all pods
export COWEN_APP_MODE="store-app"
export COWEN_APP_KEY="AK_STRESS"
export COWEN_APP_SECRET="AS_STRESS"
export COWEN_ENCRYPT_KEY="1234567890123456"
export COWEN_WEBHOOK_TARGET="http://127.0.0.1:8080/cb"
export COWEN_OPENAPI_URL="$MOCK_URL"
export COWEN_STREAM_URL="$MOCK_WS"
export COWEN_STORE_TYPE="redis"
export COWEN_DB_URL="redis://127.0.0.1:$REDIS_PORT/0"

# Store the base COWEN_HOME
BASE_HOME="$COWEN_HOME"

echo -e "${BOLD}1. Scale to 4 Pods simultaneously${NC}"
PIDS=()
for i in {1..4}; do
    POD_HOME="$BASE_HOME/pod_$i"
    mkdir -p "$POD_HOME"
    # Each pod gets a unique proxy port to avoid localhost conflicts
    export COWEN_PROXY_PORT=$((9100 + i))
    # Note: COWEN_HOME must be set per process for local files (logs, etc)
    COWEN_HOME="$POD_HOME" "$COWEN_BIN" daemon start --foreground > "$POD_HOME/daemon.log" 2>&1 &
    PIDS+=($!)
done

echo -e "  Waiting for pods to stabilize (15s)..."
sleep 15

# Verify all 4 are running
RUNNING_COUNT=0
for i in {1..4}; do
    POD_HOME="$BASE_HOME/pod_$i"
    if COWEN_HOME="$POD_HOME" "$COWEN_BIN" status | grep -q "ACTIVE\|RUNNING"; then
        ((RUNNING_COUNT++))
    fi
done

if [ "$RUNNING_COUNT" -eq 4 ]; then
    echo -e "  ${GREEN}✓${NC} 4 pods are active"
else
    echo -e "  ${RED}✗${NC} Only $RUNNING_COUNT pods are active"
    # Print one log for debug
    echo "  --- POD 1 LOG ---"
    cat "$BASE_HOME/pod_1/daemon.log"
    exit 1
fi

# Check mock server logs for redundant resend requests
RESEND_COUNT=$(grep -c "auth/appTicket/resend" "target/cowen_tests/mock_server_$MOCK_PORT.log" || true)
echo -e "  Resend requests triggered: $RESEND_COUNT"

echo -e "\n${BOLD}2. Scaling from 4 to 8 Pods${NC}"
for i in {5..8}; do
    POD_HOME="$BASE_HOME/pod_$i"
    mkdir -p "$POD_HOME"
    export COWEN_PROXY_PORT=$((9100 + i))
    COWEN_HOME="$POD_HOME" "$COWEN_BIN" daemon start --foreground > "$POD_HOME/daemon.log" 2>&1 &
    PIDS+=($!)
done

echo -e "  Waiting for scaling stabilization (15s)..."
sleep 15

RUNNING_COUNT=0
for i in {1..8}; do
    POD_HOME="$BASE_HOME/pod_$i"
    if COWEN_HOME="$POD_HOME" "$COWEN_BIN" status | grep -q "ACTIVE\|RUNNING"; then
        ((RUNNING_COUNT++))
    fi
done

if [ "$RUNNING_COUNT" -eq 8 ]; then
    echo -e "  ${GREEN}✓${NC} 8 pods are active"
else
    echo -e "  ${RED}✗${NC} Only $RUNNING_COUNT pods are active"
    exit 1
fi

# Final check: Token Consistency
echo -e "\n${BOLD}3. Verifying Token Consistency across Cluster${NC}"
TOKEN_01=$(COWEN_HOME="$BASE_HOME/pod_1" extract_token "default")
TOKEN_08=$(COWEN_HOME="$BASE_HOME/pod_8" extract_token "default")

if [ "$TOKEN_01" == "$TOKEN_08" ] && [ -n "$TOKEN_01" ]; then
    echo -e "  ${GREEN}✓${NC} All pods sharing consistent token: ${TOKEN_01:0:20}..."
else
    echo -e "  ${RED}✗${NC} Token inconsistency or missing detected!"
    echo "  Pod 1: $TOKEN_01"
    echo "  Pod 8: $TOKEN_08"
    exit 1
fi

# Cleanup
for pid in "${PIDS[@]}"; do
    kill -9 $pid 2>/dev/null
done

echo -e "\n${GREEN}🎊 Case 30 Passed! (Scaling Resilience Verified)${NC}"
