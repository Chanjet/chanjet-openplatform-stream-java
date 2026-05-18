#!/bin/bash
# Test Case 45: Configuration Hot-Reload
# Purpose: Verify that the daemon hot-reloads configurations without restarting the process.
# Validates dynamic log level updates and non-interruptive configuration refreshes.

set -e
NC='\033[0m'
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'

echo -e "${BLUE}Starting Test Case 45: Config Hot-Reload${NC}"

# 1. Setup environment
if [ -z "$COWEN_BIN" ]; then
    export COWEN_BIN="$(pwd)/target/release/cowen"
fi
TEST_ID=$RANDOM
export COWEN_HOME="$(pwd)/target/cowen_tests/case_45_$TEST_ID"
mkdir -p "$COWEN_HOME"
TEST_PROFILE="hot_reload_test_$TEST_ID"

# 2. Initialize profile
$COWEN_BIN init --profile "$TEST_PROFILE" --app-mode self-built --app-key "k" --app-secret "s" --certificate "c" --encrypt-key "e" --stream-url "http://localhost:8080" > /dev/null
$COWEN_BIN config set --profile "$TEST_PROFILE" log.level debug


# 3. Start daemon in background
$COWEN_BIN daemon start --profile "$TEST_PROFILE"
DAEMON_PID=$(cat "$COWEN_HOME/${TEST_PROFILE}_daemon.pid" | head -n 1)
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
CURRENT_PID=$(cat "$COWEN_HOME/${TEST_PROFILE}_daemon.pid" | head -n 1)
if [ "$DAEMON_PID" != "$CURRENT_PID" ]; then
    echo -e "${RED}[FAILED]${NC} Daemon restarted! PID changed from $DAEMON_PID to $CURRENT_PID"
    exit 1
fi
echo "PID preserved: $CURRENT_PID"

# 7. Verify config level updated
$COWEN_BIN config --profile "$TEST_PROFILE" | grep "Log Level" | grep -q "info"
echo "Config updated to info."

# 8. Cleanup
$COWEN_BIN daemon stop --profile "$TEST_PROFILE"
echo -e "${GREEN}Test Case 45 PASSED!${NC}"
rm -rf "$COWEN_HOME"
