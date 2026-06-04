#!/bin/bash
set -e
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
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

# Wait briefly for foreground daemon to write its PID file
sleep 1
ORIGINAL_PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")

echo -e "${BOLD}3. Concurrent start in background (simulate user CLI action)${NC}"
# This should NOT crash the existing daemon, nor crash itself. It should just send IPC or wait.
"$COWEN_BIN" daemon start --profile main > "$COWEN_HOME/background_start.log" 2>&1
assert_pass "Background start CLI command succeeded"

# Wait for daemon to become healthy
wait_for_daemon main 10
assert_pass "Daemon is running and healthy"

echo -e "${BOLD}4. Verify single master daemon process${NC}"
# Check that the PID in master_daemon.pid is still the original daemon
ACTUAL_PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")
if [ "$ACTUAL_PID" != "$ORIGINAL_PID" ]; then
    fail_suite "Expected daemon PID to remain $ORIGINAL_PID, but found $ACTUAL_PID in pidfile"
fi
# Verify the process is actually running
if ! kill -0 $ACTUAL_PID 2>/dev/null; then
    fail_suite "Daemon process $ACTUAL_PID is not running"
fi
assert_pass "Only 1 daemon process is running (PID stable)"

echo -e "${BOLD}5. Stop daemon gracefully${NC}"
"$COWEN_BIN" daemon stop --profile main >/dev/null
sleep 2

if ps -p $LAUNCHD_PID > /dev/null; then
    kill -9 $LAUNCHD_PID || true
fi

pass_suite
