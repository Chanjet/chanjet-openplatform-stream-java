#!/bin/bash
# Case 41: Init Deduplication
# Ensures that initializing with the same parameters (app_key, app_mode) 
# doesn't create a new profile even if a new name is provided.

source tests/e2e/scripts/common.sh

setup_workspace "init_dedup"
trap cleanup_suite EXIT
start_mock

# 1. Initialize first profile
echo "1. Initializing first profile 'prof_a'..."
"$COWEN_BIN" init --profile prof_a \
    --app-mode self-built \
    --app-key KEY_DUP \
    --app-secret SEC_DUP \
    --encrypt-key 1234567890123456 \
    --certificate CERT_DUP \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS >/dev/null
assert_pass "Initialized profile 'prof_a'"

# 2. Attempt to initialize second profile with SAME parameters but DIFFERENT name
echo "2. Initializing second profile 'prof_b' with SAME parameters..."
OUTPUT=$("$COWEN_BIN" init --profile prof_b \
    --app-mode self-built \
    --app-key KEY_DUP \
    --app-secret SEC_DUP \
    --encrypt-key 1234567890123456 \
    --certificate CERT_DUP \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS 2>&1)

# The command should either fail or explicitly state it's reusing an existing profile.
# For now, let's assume it should warn the user and NOT create a new profile file.

# 3. Verify 'prof_b' was NOT created
echo "3. Verifying 'prof_b' was not created..."
LIST=$("$COWEN_BIN" profile list)
if echo "$LIST" | grep -q "prof_b"; then
    echo -e "  ${RED}✗${NC} 'prof_b' should NOT have been created"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} 'prof_b' was not created"

# 4. Verify current profile is still 'prof_a' (or shifted to it)
echo "4. Checking current profile..."
CURRENT=$("$COWEN_BIN" profile current)
assert_match "$CURRENT" "prof_a" "Current profile should be 'prof_a'"

# 5. Verify the output mentions duplication
assert_match "$OUTPUT" "already exists" "Output should mention existing profile"

echo -e "\n${GREEN}🎊 Case 41 Passed!${NC}"
exit 0
