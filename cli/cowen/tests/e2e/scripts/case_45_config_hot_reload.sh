#!/bin/bash
set -e

# Case 45: Config Hot-Reload
# GIVEN: Daemon is running with log level 'info'
# WHEN: Update log level to 'debug' in config file
# THEN: Daemon should reload config and output debug logs WITHOUT restart (PID remains same)

source "$(dirname "$0")/common.sh"

# 1. Setup workspace
setup_workspace "case_45"
trap cleanup_suite EXIT
start_mock

PROFILE="hot_reload_test"
PORT=16450
LOG_FILE="$COWEN_HOME/logs/${PROFILE}_sys.log"

# 2. Setup profile (Using self-built to avoid interactive OAuth2 flow)
# 🚀 COWEN_SKIP_DAEMON_RECOVERY ensures init doesn't start a background daemon
echo "Setup profile $PROFILE..."
export COWEN_SKIP_DAEMON_RECOVERY=1
$COWEN_BIN init --profile $PROFILE \
    --app-key "HR-KEY" \
    --app-secret "HR-SECRET" \
    --app-mode "self-built" \
    --certificate "DUMMY-CERT" \
    --encrypt-key "1234567890123456" \
    --proxy-port $PORT \
    --webhook-target "http://localhost:8080" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS > /dev/null

# Ensure log level is 'info' initially
sed -i '' 's/level: debug/level: info/g' "$COWEN_HOME/$PROFILE.yaml"

# 3. Start daemon in foreground for this test
echo "Starting daemon..."
$COWEN_BIN daemon start --profile $PROFILE --foreground > "$COWEN_HOME/daemon.log" 2>&1 &
sleep 5

# Get PID from file
DAEMON_PID=$(get_daemon_pid $PROFILE)
if [ -z "$DAEMON_PID" ]; then
    echo "❌ Daemon failed to start"
    cat "$COWEN_HOME/daemon.log"
    exit 1
fi

echo "Initial PID: $DAEMON_PID"

# 4. Check initial log level (should be info)
echo "Checking initial logs..."
# Trigger some activity to generate logs
$COWEN_BIN api list --profile $PROFILE > /dev/null 2>&1 || true

# 5. Update config to 'debug'
echo "Updating log level to debug..."
sed -i '' 's/level: info/level: debug/g' "$COWEN_HOME/$PROFILE.yaml"

# 6. Wait for reload (notify + FS sync might take time)
echo "Waiting for hot-reload..."
sleep 8

# 7. Verify PID is still the same
CURRENT_PID=$(get_daemon_pid $PROFILE)
if [ "$DAEMON_PID" != "$CURRENT_PID" ]; then
    echo "❌ PID changed or daemon exited! Hot-reload failed."
    echo "Expected PID: $DAEMON_PID"
    echo "Actual PID: $CURRENT_PID"
    echo "--- Daemon Log ---"
    cat "$COWEN_HOME/daemon.log"
    exit 1
fi
echo "✅ PID remains $DAEMON_PID"

# 8. Verify debug logs are present
if grep -q "DEBUG" "$LOG_FILE"; then
    echo "✅ Debug logs found"
else
    # Try one more command to trigger debug logs
    $COWEN_BIN api list --profile $PROFILE > /dev/null 2>&1 || true
    sleep 2
    if grep -q "DEBUG" "$LOG_FILE"; then
        echo "✅ Debug logs found"
    else
        echo "❌ Debug logs NOT found after hot-reload"
        echo "--- Last 20 lines of log ---"
        tail -n 20 "$LOG_FILE"
        cleanup_suite
        exit 1
    fi
fi

# 9. Cleanup
cleanup_suite
echo "🎉 Case 45 Passed!"
