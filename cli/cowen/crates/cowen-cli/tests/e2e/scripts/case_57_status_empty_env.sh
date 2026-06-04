#!/bin/bash
set -e
# Case 57: Empty Environment Status Check
# Verifies:
#   1. Running 'cowen status' in a completely uninitialized environment.
#   2. Validates that no profile is artificially displayed.
#   3. Validates that no OAuth2/Daemon efficiency tips are thrown.
#   4. Validates that global storage is correctly printed.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Empty Environment${NC}"
setup_workspace "case_57"
# We intentionally DO NOT initialize any profiles.

echo -e "${BOLD}2. Run 'cowen status' and Check Output${NC}"
OUT=$("$COWEN_BIN" status)
echo "$OUT"

# Verify that the system explicitly states it's uninitialized
assert_match "$OUT" "System is not initialized" "Uninitialized system detected correctly"
assert_match "$OUT" "Profile: Not Initialized" "'Not Initialized' placeholder printed"

# Verify no artificial 'default' profile is displayed
assert_not_match "$OUT" "Profile: 'default'" "Artificial 'default' profile should not be printed"

# Verify that we DO NOT show Provider warnings (like "AppKey is missing" under Provider or "Efficiency Tip")
assert_not_match "$OUT" "Efficiency Tip" "No false positive Efficiency Tip warnings"
assert_not_match "$OUT" "Oauth2 Mode Diagnostics" "No false positive Oauth2 Mode Diagnostics"

# Verify global storage is still printed
assert_match "$OUT" "📦 Storage: Mode:" "Global storage mode correctly printed"

pass_suite
