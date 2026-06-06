#!/usr/bin/env bash
# case_76_status_storage_mode.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

setup_workspace "case_76_status"

echo "Initializing profile to generate innerdb storage..."
"$COWEN_BIN" -p "default" init --app-key "test-key" --app-secret "test-secret" --app-mode "self_built" --certificate "test-cert" --encrypt-key "test-key"

echo "Checking system status for storage mode..."
OUT=$("$COWEN_BIN" status)
echo "$OUT"

if ! echo "$OUT" | grep -q "Storage: Mode: innerdb"; then
    fail_suite "Status output did not display 'Storage: Mode: innerdb'"
fi

pass_suite
