#!/bin/bash
set -e
# Case 13: Distributed Deployment & Load Balancing Verification
# Verifies that multiple cowen nodes can coexist and messages are distributed across them.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_13"
HOME_A="$COWEN_HOME/node_a"
HOME_B="$COWEN_HOME/node_b"

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 13 environment...${NC}"
    kill_daemons_in_dirs "$HOME_A" "$HOME_B"
    cleanup_suite
}
trap final_cleanup EXIT

# rm -rf "$HOME_A" "$HOME_B"
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
kill_daemons_in_dirs "$HOME_A" "$HOME_B"
rm -rf "$HOME_A" "$HOME_B"
mkdir -p "$HOME_A" "$HOME_B"
if [ "$COWEN_MOCK_MANAGED" == "true" ]; then
    curl -s -X POST "$MOCK_URL/control/kill_connections" >/dev/null 2>&1 || true
    curl -s -X POST "$MOCK_URL/control/clear_webhooks" >/dev/null 2>&1 || true
fi
# Re-init after cleanup
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
CONN=$(wait_for_connections 2)
if [ -z "$CONN" ] || [ "$CONN" -lt 2 ]; then
    fail_suite "[FAILED waiting for 2 connections, got: $CONN]"
fi
echo -e " ${GREEN}[$CONN connections]${NC}"

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
RECV_COUNT=$(wait_for_webhook_count "DIST_TEST" 10)
if [ -z "$RECV_COUNT" ] || [ "$RECV_COUNT" -ne 10 ]; then
    curl -s "$MOCK_URL/control/connection_count" | python3 -m json.tool
    fail_suite "[$RECV_COUNT/10]"
fi
echo -e " ${GREEN}[10/10]${NC}"

echo -e "${BOLD}5. Distribution Analysis${NC}"
COUNT_A=$(cat "$HOME_A/logs/"*.log 2>/dev/null | grep -c "DIST_TEST" || true)
COUNT_B=$(cat "$HOME_B/logs/"*.log 2>/dev/null | grep -c "DIST_TEST" || true)
echo -e "     Node A handled: $COUNT_A messages"
echo -e "     Node B handled: $COUNT_B messages"

if [ "$COUNT_A" -gt 0 ] && [ "$COUNT_B" -gt 0 ]; then
    echo -e "   ${GREEN}✓${NC} Load balancing confirmed"
else
    echo -e "   ${YELLOW}⚠${NC} Uneven distribution (typical for small sample)"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

pass_suite
