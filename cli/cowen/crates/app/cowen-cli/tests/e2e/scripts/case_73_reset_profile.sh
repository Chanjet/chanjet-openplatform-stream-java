#!/usr/bin/env bash
# case_73_reset_profile.sh
# Tests the profile-specific reset functionality (cowen reset -p x)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_73_main"
setup_workspace "case_73_$PROFILE"

# 1. Initialize main profile
echo "Creating main profile..."
"$COWEN_BIN" -p "$PROFILE" init --app-key "main-key" --app-secret "main-secret" --certificate "main-cert" --app-mode "self_built" --encrypt-key "main-encrypt-key"
"$COWEN_BIN" profile use "$PROFILE"

# 2. Initialize secondary profile
SECONDARY_PROFILE="case_73_sec"
echo "Creating secondary profile..."
"$COWEN_BIN" -p "$SECONDARY_PROFILE" init --app-key "sec-key" --app-secret "sec-secret" --certificate "sec-cert" --app-mode "self_built" --encrypt-key "sec-encrypt-key"

# Switch back to main to test resetting a non-current profile
"$COWEN_BIN" profile use "$PROFILE"

# Verify both profiles exist
if [ ! -f "$COWEN_HOME/$PROFILE.yaml" ]; then
    fail_suite "Main profile config missing!"
fi
if [ ! -f "$COWEN_HOME/$SECONDARY_PROFILE.yaml" ]; then
    fail_suite "Secondary profile config missing!"
fi

# 2.5 Initialize third profile
THIRD_PROFILE="case_73_third"
echo "Creating third profile..."
"$COWEN_BIN" -p "$THIRD_PROFILE" init --app-key "third-key" --app-secret "third-secret" --certificate "third-cert" --app-mode "self_built" --encrypt-key "third-encrypt-key"

# Switch back to main to test resetting a non-current profile
"$COWEN_BIN" profile use "$PROFILE"


# 3. Reset secondary profile
echo "Running reset on secondary profile..."
"$COWEN_BIN" reset -p "$SECONDARY_PROFILE"

# 4. Verify cowen_home files for secondary profile are deleted
if [ -f "$COWEN_HOME/$SECONDARY_PROFILE.yaml" ]; then
    fail_suite "Secondary profile config was NOT deleted!"
fi
if [ -f "$COWEN_HOME/$SECONDARY_PROFILE.db" ]; then
    fail_suite "Secondary profile vault DB was NOT deleted!"
fi

# 5. Verify main profile still exists
if [ ! -f "$COWEN_HOME/$PROFILE.yaml" ]; then
    fail_suite "Main profile config was accidentally deleted!"
fi

# 6. Verify status --all and profile list do not contain secondary profile
echo "Checking status --all..."
STATUS_OUT=$("$COWEN_BIN" status --all -o text)
if echo "$STATUS_OUT" | grep -q "$SECONDARY_PROFILE"; then
    fail_suite "status --all still shows the deleted secondary profile!"
fi

echo "Checking profile list..."
PROFILE_LIST_OUT=$("$COWEN_BIN" profile list)
if echo "$PROFILE_LIST_OUT" | grep -q "$SECONDARY_PROFILE"; then
    fail_suite "profile list still shows the deleted secondary profile!"
fi

# 7. Verify current profile is NOT reset since we deleted a different one
CURRENT=$("$COWEN_BIN" profile current)
if ! echo "$CURRENT" | grep -q "$PROFILE"; then
    fail_suite "Current profile changed unexpectedly! Expected $PROFILE, got $CURRENT"
fi

# 8. Reset the main profile (which is the current profile)
echo "Running reset on current profile..."
"$COWEN_BIN" reset -p "$PROFILE"

# 9. Verify current profile falls back to the third profile
CURRENT_AFTER=$("$COWEN_BIN" profile current)
if ! echo "$CURRENT_AFTER" | grep -q "$THIRD_PROFILE"; then
    fail_suite "Current profile did not fallback to an available profile! Expected $THIRD_PROFILE, Got $CURRENT_AFTER"
fi

# 10. Verify third profile is usable
echo "Verifying third profile is usable..."
"$COWEN_BIN" profile list

# 11. Reset the third profile
echo "Running reset on third profile..."
"$COWEN_BIN" reset -p "$THIRD_PROFILE"

# 12. Verify current profile falls back to default when no profiles exist
CURRENT_FINAL=$("$COWEN_BIN" profile current)
if ! echo "$CURRENT_FINAL" | grep -q "default"; then
    fail_suite "Current profile did not fallback to 'default' when no profiles existed! Got $CURRENT_FINAL"
fi

echo "✅ Profile reset test Passed!"
