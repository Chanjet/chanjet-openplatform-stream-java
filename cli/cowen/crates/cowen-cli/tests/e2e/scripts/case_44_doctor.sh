#!/bin/bash
# Test Case 44: Cowen Doctor
# Purpose: Verify that the cowen doctor command correctly diagnoses the environment.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_44"
trap cleanup_suite EXIT

TEST_PROFILE="doctor_test_prof"

# 2. Run doctor on uninitialized profile (defaults to oauth2 mode)
# OAuth2 mode does NOT require app_secret — it uses built-in client ID with PKCE.
# Credentials check should NOT report "App Secret 缺失".
echo "Running doctor on uninitialized oauth2 profile..."
$COWEN_BIN doctor --profile "$TEST_PROFILE" > doctor_output.log || true
if grep -q "App Secret 缺失" doctor_output.log; then
    cat doctor_output.log
    fail_suite "OAuth2 mode should NOT report 'App Secret 缺失'."
fi
echo "OAuth2 mode credentials check passed (no App Secret error)."

# 3. Initialize self-built with valid 16-byte key and run doctor
echo "Initializing self-built profile with valid key..."
SELFBUILT_PROFILE="doctor_selfbuilt_prof"
$COWEN_BIN init --profile "$SELFBUILT_PROFILE" --app-mode self-built --app-key "k" --app-secret "1234567890123456" --certificate "c" --encrypt-key "1234567890123456" --stream-url "http://localhost:8080" > /dev/null

$COWEN_BIN doctor --profile "$SELFBUILT_PROFILE" > doctor_selfbuilt.log || true
if grep -q "缺少解密密钥" doctor_selfbuilt.log || grep -q "解密密钥不合规" doctor_selfbuilt.log; then
    cat doctor_selfbuilt.log
    fail_suite "Doctor reported decrypt key error for self-built profile with valid key."
fi
echo "Self-built credentials check passed with valid key."

# 4. Verify network checks are present
if grep -q "OpenAPI" doctor_selfbuilt.log; then
    echo "Network check included OpenAPI."
else
    cat doctor_selfbuilt.log
    fail_suite "Doctor output missing network check."
fi

echo -e "${GREEN}Test Case 44 PASSED!${NC}"
rm -f doctor_output.log doctor_selfbuilt.log
