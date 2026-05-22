#!/bin/bash
set -e
# Case 12: Daemon Recovery Verification
# Checks if the daemon is automatically restarted when a command is executed after a crash.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

# Setup workspace
setup_workspace "case_12"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_RECOVERY \
    --app-secret AS_RECOVERY \
    --encrypt-key 1234567890123456 \
    --certificate CERT_RECOVERY \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Initial Daemon${NC}"
"$COWEN_BIN" status >/dev/null # Trigger first start
sleep 2

# Get initial PID
PID1=$(get_daemon_pid "main")
if [ -n "$PID1" ]; then
    echo -e "   ${GREEN}✓${NC} Initial daemon running (PID: $PID1)"
else
    fail_suite "Failed to start initial daemon"
fi

echo -e "${BOLD}3. Simulate Daemon Crash${NC}"
echo "   DEBUG: PID1=$PID1"
kill -9 "$PID1"
EXIT_CODE=$?
echo -e "   ${YELLOW}⚡ Daemon process (PID: $PID1) killed manually (Exit: $EXIT_CODE)${NC}"

# Verify it's gone
sleep 1
if ! kill -0 "$PID1" 2>/dev/null; then
    echo -e "   ${GREEN}✓${NC} Process confirmed dead"
else
    ps -p "$PID1" || echo "ps command failed"
    fail_suite "Process still exists!"
fi

echo -e "${BOLD}4. Trigger Recovery via Command${NC}"
echo -e "   Running 'cowen status' to trigger recovery..."
STATUS_OUTPUT=$("$COWEN_BIN" status)

# Check for recovery message in output or just check PID
PID2=$(get_daemon_pid "main")

if [ -n "$PID2" ] && [ "$PID1" != "$PID2" ]; then
    echo -e "   ${GREEN}✓${NC} New daemon started (PID: $PID2)"
else
    echo -e "   ${RED}✗${NC} Daemon recovery failed"
    fail_suite "$STATUS_OUTPUT"
fi

echo -e "${BOLD}5. Verify Connection Stability after Recovery${NC}"
# Wait for daemon to be running and reported in status
if wait_for_daemon_status "" "\[RUNNING\]" 10; then
    RUNNING=true
else
    RUNNING=false
fi


if [ "$RUNNING" = true ]; then
    echo -e "   ${GREEN}✓${NC} Bridge re-connected after recovery"
else
    "$COWEN_BIN" status
    fail_suite "Bridge failed to connect after recovery"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 12 Passed!${NC}"
cleanup_suite
