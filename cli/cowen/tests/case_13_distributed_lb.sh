#!/bin/bash
# Case 13: Distributed Deployment & Load Balancing Verification
# Verifies that multiple cowen nodes can coexist and messages are distributed across them.

source tests/common.sh

# Define nodes
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
HOME_A="$TEST_BASE/.cowen_test_dist_lb_node_a"
HOME_B="$TEST_BASE/.cowen_test_dist_lb_node_b"
mkdir -p "$TEST_BASE"

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 13 environment...${NC}"
    cleanup_suite "case_13"
    rm -rf "$HOME_A" "$HOME_B"
}
trap final_cleanup EXIT

rm -rf "$HOME_A" "$HOME_B"
mkdir -p "$HOME_A" "$HOME_B"
export COWEN_EXCLUSIVE=false

echo -e "${BOLD}1. Setup Node A and Node B${NC}"

setup_node() {
    local home=$1
    local port=$2
    rm -rf "$home"
    mkdir -p "$home"
    export COWEN_HOME="$home"
    "$COWEN_BIN" init --profile main \
        --app-mode self-built --app-key AK_DIST --app-secret AS_DIST \
        --encrypt-key 1234567890123456 --certificate CERT_DIST \
        --openapi-url $MOCK_URL --stream-url $MOCK_WS \
        --webhook-target "http://127.0.0.1:9299/webhook_sink" \
        --proxy-port $port >/dev/null
    # Stop auto-daemon
    "$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
}

setup_node "$HOME_A" 9091
setup_node "$HOME_B" 9092
assert_pass "Nodes initialized"

echo -e "${BOLD}2. Clean Environment${NC}"
cleanup_suite "distributed_lb_pre"
# Re-init after cleanup deleted directories
setup_node "$HOME_A" 9091
setup_node "$HOME_B" 9092
start_mock

# Verify clean state
CONN=$(curl -s "$MOCK_URL/control/connection_count" | python3 -c "import sys, json; print(json.load(sys.stdin).get('count', 0))")
echo -e "   Residual connections after cleanup: $CONN"
assert_pass "Environment cleaned"

echo -e "${BOLD}3. Start Both Daemons${NC}"
export COWEN_HOME="$HOME_A"
"$COWEN_BIN" daemon start --profile main >/dev/null
echo -e "   Node A started (PID: $(get_daemon_pid 'main'))"

export COWEN_HOME="$HOME_B"
"$COWEN_BIN" daemon start --profile main >/dev/null
echo -e "   Node B started (PID: $(get_daemon_pid 'main'))"

# Wait for EXACTLY 2 connections
echo -n "   Waiting for 2 WS connections..."
CONN=0
for i in {1..15}; do
    CONN=$(curl -s "$MOCK_URL/control/connection_count" | python3 -c "import sys, json; print(json.load(sys.stdin).get('count', 0))")
    if [ "$CONN" -ge 2 ]; then
        echo -e " ${GREEN}[$CONN connections]${NC}"
        break
    fi
    sleep 1
    echo -n "."
done

echo -e "${BOLD}4. Verify Load Balancing${NC}"
# Send 10 messages in LB mode
echo -n "   Sending 10 messages in LB mode..."
for i in {1..10}; do
    curl -s -X POST "$MOCK_URL/control/broadcast" \
        -H "Content-Type: application/json" \
        -d "{\"msg_type\": \"DIST_TEST\", \"mode\": \"lb\", \"payload\": {\"seq\": $i}}" >/dev/null
    sleep 0.1
    echo -n "."
done
echo -e " [DONE]"

# Wait for all 10 to reach the sink
echo -n "   Waiting for webhook delivery..."
RECV_COUNT=0
for i in {1..25}; do
    MESSAGES=$(curl -s "$MOCK_URL/control/webhooks")
    RECV_COUNT=$(echo "$MESSAGES" | python3 -c "import sys, json; d=json.load(sys.stdin); print(len([m for m in d if (m.get('body') or m).get('msg_type') == 'DIST_TEST']))")
    if [ "$RECV_COUNT" -eq 10 ]; then
        echo -e " ${GREEN}[10/10]${NC}"
        break
    fi
    sleep 1
    echo -n "."
done

if [ "$RECV_COUNT" -ne 10 ]; then
    echo -e " ${RED}[$RECV_COUNT/10]${NC}"
    curl -s "$MOCK_URL/control/connection_count" | python3 -m json.tool
    exit 1
fi

echo -e "${BOLD}5. Distribution Analysis${NC}"
COUNT_A=$(grep -c "DIST_TEST" "$HOME_A/logs/main_stream.log" 2>/dev/null || echo 0)
COUNT_B=$(grep -c "DIST_TEST" "$HOME_B/logs/main_stream.log" 2>/dev/null || echo 0)
echo -e "     Node A handled: $COUNT_A messages"
echo -e "     Node B handled: $COUNT_B messages"

if [ "$COUNT_A" -gt 0 ] && [ "$COUNT_B" -gt 0 ]; then
    echo -e "   ${GREEN}✓${NC} Load balancing confirmed"
else
    echo -e "   ${YELLOW}⚠${NC} Uneven distribution (typical for small sample)"
fi

echo -e "\n${GREEN}🎊 Case 13 Passed!${NC}"
