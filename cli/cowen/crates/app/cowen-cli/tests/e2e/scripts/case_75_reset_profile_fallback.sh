#!/usr/bin/env bash
# case_75_reset_profile_fallback.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

setup_workspace "case_75_reset"

echo "1. Initialize two profiles: profile_a and profile_b"

"$COWEN_BIN" -p profile_a init --app-key "key_a" --app-secret "sec_a" --app-mode "self_built" --certificate "test-cert" --encrypt-key "test-key"
"$COWEN_BIN" -p profile_b init --app-key "key_b" --app-secret "sec_b" --app-mode "self_built" --certificate "test-cert" --encrypt-key "test-key"

echo "Set current profile to profile_a"
"$COWEN_BIN" profile use profile_a

echo "2. Verify both profiles exist"
LIST_OUT=$("$COWEN_BIN" profile list)
if ! echo "$LIST_OUT" | grep -q "profile_a"; then fail_suite "profile_a missing"; fi
if ! echo "$LIST_OUT" | grep -q "profile_b"; then fail_suite "profile_b missing"; fi

echo "3. Reset current profile (profile_a)"
"$COWEN_BIN" reset -p profile_a

echo "4. Verify profile_a disappeared from profile list"
LIST_OUT_AFTER=$("$COWEN_BIN" profile list)
if echo "$LIST_OUT_AFTER" | grep -q "profile_a"; then fail_suite "profile_a should be deleted"; fi
if ! echo "$LIST_OUT_AFTER" | grep -q "profile_b"; then fail_suite "profile_b should still exist"; fi

echo "5. Verify profile_a disappeared from status --all"
STATUS_ALL=$("$COWEN_BIN" status --all)
if echo "$STATUS_ALL" | grep -q "Profile: 'profile_a'"; then fail_suite "profile_a should not be in status --all"; fi

echo "6. Verify config files for profile_a are deleted in COWEN_HOME"
if [ -f "$COWEN_HOME/profile_a.yaml" ]; then
    fail_suite "profile_a.yaml was not deleted"
fi

echo "7. Verify current profile changed to a usable profile (profile_b or default)"
CURRENT=$("$COWEN_BIN" profile current)
echo "Current profile after reset: $CURRENT"
if [ "$CURRENT" == "profile_a" ]; then
    fail_suite "Current profile did not change after profile_a was reset"
fi

echo "8. Verifying the fallback profile is usable..."
"$COWEN_BIN" status

pass_suite
