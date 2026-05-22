#!/usr/bin/env bash
# case_56_version_json.sh
# Tests the structured version output for monitoring integration

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_56_version"
setup_workspace "case_56_$PROFILE"

echo "Checking cowen version -o json..."
JSON_OUT=$("$COWEN_BIN" version -o json)

# Ensure the output is valid JSON and contains required keys
if ! echo "$JSON_OUT" | jq -e '.build_id' > /dev/null; then
    fail_suite "Missing build_id in JSON output"
fi

if ! echo "$JSON_OUT" | jq -e '.build_time' > /dev/null; then
    fail_suite "Missing build_time in JSON output"
fi

if ! echo "$JSON_OUT" | jq -e '.version' > /dev/null; then
    fail_suite "Missing version in JSON output"
fi

echo "✅ JSON Output:"
echo "$JSON_OUT" | jq .

echo "✅ Version JSON test Passed!"
