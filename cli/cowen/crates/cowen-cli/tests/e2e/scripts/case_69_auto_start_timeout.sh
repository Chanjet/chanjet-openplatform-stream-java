#!/bin/bash
set -e
source "$(dirname "$0")/common.sh"

export COWEN_HOME="$(mktemp -d)"
export COWEN_PROFILE="default"
trap 'rm -rf "$COWEN_HOME"' EXIT

# Initialize a dummy profile so it gets listed in auto_start_all
"$COWEN_BIN" init --app-mode store_app --app-key dummy --app-secret dummy --encrypt-key dummy

export COWEN_MOCK_SLOW_START_DAEMON=25

# 1. Start the daemon with a slow startup worker
# --auto-start is implicit or we can specify it
# We'll just run `cowen status`, which will spawn the daemon and trigger auto-start.
echo "Running cowen status (which spawns daemon)..."
start_time=$(date +%s)
"$COWEN_BIN" status
end_time=$(date +%s)
duration=$((end_time - start_time))

if [ "$duration" -gt 5 ]; then
    echo "cowen status took $duration seconds! It should return immediately while the worker starts in the background."
    exit 1
fi

echo "cowen status returned immediately. Worker is starting in background."

# Cleanup
echo "Stopping daemon..."
"$COWEN_BIN" daemon stop

echo "Test case 69 Passed!"
exit 0
