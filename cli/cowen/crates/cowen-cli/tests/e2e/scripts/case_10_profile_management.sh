#!/bin/bash
set -e
# Case 10: Profile Management (use, rename, list, current, reset)

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_10"
trap cleanup_suite EXIT
start_mock

# 1. Initialize multiple profiles
echo "1. Initializing multiple profiles..."
"$COWEN_BIN" init --profile prof_a \
    --app-mode self-built \
    --app-key KEY_A \
    --app-secret SEC_A \
    --encrypt-key 1234567890123456 \
    --certificate CERT_A \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS >/dev/null
assert_pass "Initialized profile 'prof_a'"

"$COWEN_BIN" init --profile prof_b \
    --app-mode self-built \
    --app-key KEY_B \
    --app-secret SEC_B \
    --encrypt-key 1234567890123456 \
    --certificate CERT_B \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS >/dev/null
assert_pass "Initialized profile 'prof_b'"

# 2. Check current profile (should be default or prof_b if last initialized?)
# Actually 'init --profile X' doesn't necessarily switch the global default unless 'use' is called.
# Let's check 'profile current'.
echo "2. Checking current profile..."
CURRENT=$("$COWEN_BIN" profile current)
# By default, it might be 'default'.
echo "   Current profile is: $CURRENT"

# 3. List profiles
echo "3. Listing profiles..."
LIST=$("$COWEN_BIN" profile list)
assert_match "$LIST" "prof_a" "List contains 'prof_a'"
assert_match "$LIST" "prof_b" "List contains 'prof_b'"

# 4. Use profile
echo "4. Switching to 'prof_a'..."
"$COWEN_BIN" profile use prof_a >/dev/null
assert_pass "Switched to 'prof_a'"

CURRENT=$("$COWEN_BIN" profile current)
assert_match "$CURRENT" "prof_a" "Current profile is now 'prof_a'"

echo "4.1 Trying to switch to an invalid profile..."
if "$COWEN_BIN" profile use nonexistent_profile 2>/dev/null; then
    fail_suite "Should not allow switching to a non-existent profile"
fi
echo -e "  ${GREEN}✓${NC} Blocked switching to 'nonexistent_profile'"

CURRENT=$("$COWEN_BIN" profile current)
assert_match "$CURRENT" "prof_a" "Current profile should still be 'prof_a'"

# Verify data in 'prof_a'
echo "   Verifying data in 'prof_a'..."
CFG=$("$COWEN_BIN" config)
assert_match "$CFG" "KEY_A" "Config in 'prof_a' has KEY_A"

# 5. Rename profile
echo "5. Renaming 'prof_b' to 'prof_c'..."
"$COWEN_BIN" profile rename prof_b prof_c >/dev/null
assert_pass "Renamed 'prof_b' to 'prof_c'"

LIST=$("$COWEN_BIN" profile list)
assert_match "$LIST" "prof_c" "List contains 'prof_c'"
if echo "$LIST" | grep -q "prof_b"; then
    fail_suite "'prof_b' should not exist anymore"
fi
echo -e "  ${GREEN}✓${NC} 'prof_b' removed from list"

# 6. Verify data migrated in rename
echo "6. Verifying data in 'prof_c' after rename..."
"$COWEN_BIN" profile use prof_c >/dev/null
CFG=$("$COWEN_BIN" config)
assert_match "$CFG" "KEY_B" "Config in 'prof_c' has KEY_B (data migrated)"

# 7. Reset profile
echo "7. Resetting 'prof_a'..."
# 'reset' command resets the CURRENT profile.
"$COWEN_BIN" profile use prof_a >/dev/null
"$COWEN_BIN" reset >/dev/null
assert_pass "Reset current profile ('prof_a')"

# Verify 'prof_a' is empty/default
# After reset, the file should be gone or contain defaults.
# The 'config' command might fail or return defaults.
CFG=$("$COWEN_BIN" config)
if echo "$CFG" | grep -q "KEY_A"; then
    fail_suite "'prof_a' should be reset (KEY_A still present)"
fi
echo -e "  ${GREEN}✓${NC} 'prof_a' data cleared"

# 8. Check list again
LIST=$("$COWEN_BIN" profile list)
# Reset doesn't delete the profile name from list if the file still exists or was recreated as default.
# But usually reset means cleaning up the vault/file.
# If the file is deleted, it might not show up in list.
# Let's see behavior.


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile prof_a 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 10 Passed!${NC}"
exit 0
