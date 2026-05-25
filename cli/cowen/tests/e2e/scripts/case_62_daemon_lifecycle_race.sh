#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_62"
trap cleanup_suite EXIT

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start daemon in foreground (simulate launchd)${NC}"
# Redirect output so it doesn't pollute the test logs
"$COWEN_BIN" daemon start --profile main --foreground > "$COWEN_HOME/launchd.log" 2>&1 &
LAUNCHD_PID=$!

# Give it a fraction of a second to simulate the port binding / startup delay
sleep 0.2

echo -e "${BOLD}3. Concurrent start in background (simulate user CLI action)${NC}"
# This should NOT crash the existing daemon, nor crash itself. It should just send IPC or wait.
"$COWEN_BIN" daemon start --profile main > "$COWEN_HOME/background_start.log" 2>&1
assert_pass "Background start CLI command succeeded"

# Wait for daemon to become healthy
wait_for_daemon main 10
assert_pass "Daemon is running and healthy"

echo -e "${BOLD}4. Verify single master daemon process${NC}"
# Check how many cowen-daemon processes are running for this workspace
DAEMON_COUNT=$(ps auxww | grep "cowen-daemon" | grep "$COWEN_HOME/uds.sock" | grep -v grep | wc -l | tr -d ' ')
if [ "$DAEMON_COUNT" != "1" ]; then
    fail_suite "Expected exactly 1 daemon process, found $DAEMON_COUNT"
fi
assert_pass "Only 1 daemon process is running"

echo -e "${BOLD}5. Stop daemon gracefully${NC}"
"$COWEN_BIN" daemon stop --profile main >/dev/null
sleep 2

if ps -p $LAUNCHD_PID > /dev/null; then
    kill -9 $LAUNCHD_PID || true
fi

pass_suite
