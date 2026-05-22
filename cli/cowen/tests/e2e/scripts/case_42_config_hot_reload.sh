#!/bin/bash
# Test Case 42: Configuration Hot-Reload
# Purpose: Verify that the daemon hot-reloads configurations without restarting the process.
# Validates dynamic log level updates and non-interruptive configuration refreshes.

set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_42"
TEST_PROFILE="hot_reload_test"

echo -e "${BLUE}Starting Test Case 42: Config Hot-Reload${NC}"

# 2. Initialize profile
$COWEN_BIN init --profile "$TEST_PROFILE" --app-mode self-built --app-key "k" --app-secret "s" --certificate "c" --encrypt-key "e" --stream-url "http://localhost:8080" > /dev/null
$COWEN_BIN config set --profile "$TEST_PROFILE" log.level debug


# 3. Start daemon in background
$COWEN_BIN daemon start --profile "$TEST_PROFILE"
wait_for_daemon "$TEST_PROFILE" 5
DAEMON_PID=$(cat "$COWEN_HOME/master_daemon.pid" | head -n 1)
echo "Daemon started with PID: $DAEMON_PID"

# 4. Verify initial log level (debug)
# We expect to see 'debug' level logs if set correctly, or verify via config check
$COWEN_BIN config --profile "$TEST_PROFILE" | grep "Log Level" | grep -q "debug"
echo "Initial config verified."

# 5. Hot-reload log level to 'info'
echo "Updating log level to 'info' via config..."
$COWEN_BIN config set --profile "$TEST_PROFILE" log.level info
sleep 2 # Give watcher time to react

# 6. Verify PID is the same
CURRENT_PID=$(cat "$COWEN_HOME/master_daemon.pid" | head -n 1)
if [ "$DAEMON_PID" != "$CURRENT_PID" ]; then
    fail_suite "Daemon restarted! PID changed from $DAEMON_PID to $CURRENT_PID"
fi
echo "PID preserved: $CURRENT_PID"

# 7. Verify config level updated
$COWEN_BIN config --profile "$TEST_PROFILE" | grep "Log Level" | grep -q "info"
echo "Config updated to info."

# 8. Cleanup
$COWEN_BIN daemon stop --profile "$TEST_PROFILE"
pass_suite
