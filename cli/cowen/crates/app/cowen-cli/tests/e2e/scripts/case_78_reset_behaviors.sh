#!/usr/bin/env bash
# case_78_reset_behaviors.sh
# Merges the behaviors previously split across case_64_reset_*.sh scripts
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

setup_workspace "case_78_reset_behaviors"

# --- Test 1: Reset specific profile ---
PROFILE1="p1"
echo "=== Test 1: Reset specific profile ==="
echo "1.1 Initialize p1 with self_built mode"
"$COWEN_BIN" init -p "$PROFILE1" --app-mode self_built --app-key AK_P1 --app-secret AS_P1 --encrypt-key 1234567890123456 --certificate CERT

echo "1.2 Check if p1 config exists"
if [ ! -f "$COWEN_HOME/$PROFILE1.yaml" ]; then
    fail_suite "Profile $PROFILE1.yaml not found after init!"
fi

echo "1.3 Reset p1"
"$COWEN_BIN" reset -p "$PROFILE1"

echo "1.4 Check if p1 config was deleted"
if [ -f "$COWEN_HOME/$PROFILE1.yaml" ]; then
    fail_suite "Profile $PROFILE1.yaml was NOT deleted!"
fi

echo "1.5 Check profile list (should not contain p1)"
if "$COWEN_BIN" profile list | grep -q "$PROFILE1"; then
    fail_suite "Profile $PROFILE1 still exists in profile list!"
fi


# --- Test 2: Missing keys after reset ---
PROFILE2="p2"
echo "=== Test 2: Missing keys after reset ==="
echo "2.1 Initialize p2 with self_built mode"
"$COWEN_BIN" init -p "$PROFILE2" --app-mode self_built --app-key AK_P2 --app-secret AS_P2 --encrypt-key 1234567890123456 --certificate CERT

echo "2.2 Reset p2"
"$COWEN_BIN" reset -p "$PROFILE2"

echo "2.3 Init p2 again with self_built but NO keys (should fail because keys were deleted)"
set +e
"$COWEN_BIN" init -p "$PROFILE2" --app-mode self_built > /dev/null 2>&1
EXIT_CODE=$?
set -e

if [ $EXIT_CODE -eq 0 ]; then
    fail_suite "Init without keys succeeded unexpectedly! It should fail because keys were reset."
fi


# --- Test 3: Full Reset ---
PROFILE3="p3"
echo "=== Test 3: Full Reset ==="
echo "3.1 Initialize p3 with self_built mode"
"$COWEN_BIN" init -p "$PROFILE3" --app-mode self_built --app-key AK_P3 --app-secret AS_P3 --encrypt-key 1234567890123456 --certificate CERT

echo "3.2 Reset full system"
"$COWEN_BIN" reset

echo "3.3 Check if p3 config was deleted"
if [ -f "$COWEN_HOME/$PROFILE3.yaml" ]; then
    fail_suite "Profile $PROFILE3.yaml was NOT deleted during full reset!"
fi

echo "3.4 Check profile list (should fail to reach daemon, or be empty)"
# After a full reset, the daemon should be killed.
# The command might fail, or it might auto-start the daemon.
# If it auto-starts, it should have no profiles.
if "$COWEN_BIN" profile list | grep -q "$PROFILE3"; then
    fail_suite "Profile $PROFILE3 still exists in profile list after full reset!"
fi

pass_suite
