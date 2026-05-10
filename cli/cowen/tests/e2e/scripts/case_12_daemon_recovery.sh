#!/bin/bash
# Case 12: Daemon Recovery Verification
# Checks if the daemon is automatically restarted when a command is executed after a crash.

source tests/e2e/scripts/common.sh

# Setup workspace
setup_workspace "daemon_recovery"
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
    --proxy-port 9091 >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Initial Daemon${NC}"
"$COWEN_BIN" status >/dev/null # Trigger first start
sleep 2

# Get initial PID
PID1=$(get_daemon_pid "main")
if [ -n "$PID1" ]; then
    echo -e "   ${GREEN}✓${NC} Initial daemon running (PID: $PID1)"
else
    echo -e "   ${RED}✗${NC} Failed to start initial daemon"
    exit 1
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
    echo -e "   ${RED}✗${NC} Process still exists!"
    ps -p "$PID1" || echo "ps command failed"
    exit 1
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
    echo "$STATUS_OUTPUT"
    exit 1
fi

echo -e "${BOLD}5. Verify Connection Stability after Recovery${NC}"
# Wait for bridge to connect
COUNT=0
CONNECTED=false
while [ $COUNT -lt 10 ]; do
    if "$COWEN_BIN" status | grep -q "Connected"; then
        CONNECTED=true
        break
    fi
    sleep 1
    COUNT=$((COUNT+1))
done

if [ "$CONNECTED" = true ]; then
    echo -e "   ${GREEN}✓${NC} Bridge re-connected after recovery"
else
    echo -e "   ${RED}✗${NC} Bridge failed to connect after recovery"
    "$COWEN_BIN" status
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 12 Passed!${NC}"
cleanup_suite
