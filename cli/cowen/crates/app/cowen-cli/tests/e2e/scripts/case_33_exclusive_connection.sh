#!/bin/bash
set -e
# Case 33: Exclusive Connection Mode Verification
# Verifies that when a new connection is established for the same AppKey in exclusive mode, 
# the previous connection is evicted.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)
PROXY_PORT_2=$(get_unused_port)

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_33"
trap cleanup_suite EXIT
start_mock

# Explicitly enable exclusive mode for this test
export COWEN_EXCLUSIVE=true

# Initialize Profile
"$COWEN_BIN" init --profile p1 \
    --app-mode self-built --app-key AK_EXCLUSIVE --app-secret AS_EXC \
    --encrypt-key 1234567890123456 --certificate CERT_EXC \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile P1 initialized"

echo -e "${BOLD}2. Start First Daemon (P1)${NC}"
"$COWEN_BIN" daemon start --profile p1 >/dev/null
wait_for_daemon p1 10
"$COWEN_BIN" auth login --profile p1 --force >/dev/null
sleep 2

# Verify P1 is connected
CONN_COUNT=$(wait_for_connections 1 5)
if [ "$CONN_COUNT" -eq 1 ]; then
    echo -e "   ${GREEN}✓${NC} First connection established"
else
    fail_suite "Failed to establish first connection (Count: $CONN_COUNT)"
fi

echo -e "${BOLD}3. Start Second Daemon (P2) with same AppKey${NC}"
# Use a different COW_HOME for P2 to simulate a different instance/node
HOME_P1=$COWEN_HOME
HOME_P2="$TEST_BASE/.cowen_test_exclusive_p2"
rm -rf "$HOME_P2"
mkdir -p "$HOME_P2"

export COWEN_HOME="$HOME_P2"
# Reuse same config but in different workspace, allocating a new monitor port
cp "$HOME_P1/app.yaml" "$HOME_P2/app.yaml"
P2_MONITOR_PORT=$(get_unused_port)
sed -i.bak "s/monitor_port:.*/monitor_port: $P2_MONITOR_PORT/" "$HOME_P2/app.yaml"
"$COWEN_BIN" init --profile p2 \
    --app-mode self-built --app-key AK_EXCLUSIVE --app-secret AS_EXC \
    --encrypt-key 1234567890123456 --certificate CERT_EXC \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT_2 >/dev/null

# Start P2
"$COWEN_BIN" daemon start --profile p2 >/dev/null
wait_for_daemon p2 10
export COWEN_HOME="$HOME_P2"
"$COWEN_BIN" auth login --profile p2 --force >/dev/null
sleep 2
echo "   P2 starting..."

echo -e "${BOLD}4. Verify Eviction Logic${NC}"
# In exclusive mode, only ONE should be active (the latest one)
# The mock server log should show an eviction
grep -q "Exclusive Eviction" "$TEST_BASE/mock_server_$MOCK_PORT.log"
assert_pass "Server log confirms exclusive eviction"

# Check connection count
CONN_COUNT=$(wait_for_connections 1 5)
if [ "$CONN_COUNT" -eq 1 ]; then
    echo -e "   ${GREEN}✓${NC} Only one connection remains active (Exclusive mode working)"
else
    # Show clients for debugging
    curl -s "$MOCK_URL/control/connection_count" | python3 -m json.tool
    fail_suite "Multiple connections detected in exclusive mode! (Count: $CONN_COUNT)"
fi

# Cleanup P2
export COWEN_HOME="$HOME_P2"
cleanup_suite
export COWEN_HOME="$HOME_P1"


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile p1 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 33 Passed!${NC}"
