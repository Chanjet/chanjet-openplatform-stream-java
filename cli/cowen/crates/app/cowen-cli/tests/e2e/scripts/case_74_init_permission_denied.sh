#!/bin/bash
set -e

# Source common testing utilities
source "$(dirname "$0")/common.sh"

setup_workspace "case_74"

echo "1. Setup Environment without write permissions"
# Simulate a root-owned or read-only ~/.cowen by removing write permissions
chmod -w "$COWEN_HOME"

echo "2. Run cowen init and verify it produces a clear error message"

# It should fail, so we don't use set -e for this command
set +e
output=$("$COWEN_BIN" init 2>&1)
exit_code=$?
set -e

if [ $exit_code -eq 0 ]; then
    echo "❌ Expected cowen init to fail due to lack of write permissions, but it succeeded!"
    # restore permission to allow cleanup
    chmod +w "$COWEN_HOME"
    exit 1
fi

echo "$output"

# We expect a clear message about permission denied or failing to create the logs directory
if echo "$output" | grep -q "Failed to create daemon logs directory"; then
    echo "  ✓ Proper error message printed for missing permissions."
elif echo "$output" | grep -q "Permission denied"; then
    echo "  ✓ Permission denied message printed."
else
    echo "❌ Expected 'Failed to create daemon logs directory' or 'Permission denied' in output, but got:"
    echo "$output"
    # restore permission to allow cleanup
    chmod +w "$COWEN_HOME"
    exit 1
fi

# Ensure it DOES NOT print the misleading 'No such file or directory (os error 2)'
if echo "$output" | grep -q "No such file or directory (os error 2)"; then
    echo "❌ Still printing the confusing 'No such file or directory (os error 2)' message!"
    chmod +w "$COWEN_HOME"
    exit 1
fi

echo "🎊 Case 74 Passed!"

# Restore permissions so cleanup can remove the directory
chmod +w "$COWEN_HOME"
