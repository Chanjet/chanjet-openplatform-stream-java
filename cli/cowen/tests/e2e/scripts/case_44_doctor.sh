#!/bin/bash
# Test Case 44: Cowen Doctor
# Purpose: Verify that the cowen doctor command correctly diagnoses the environment.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "doctor_test"
trap cleanup_suite EXIT

TEST_PROFILE="doctor_test_prof"

# 2. Run doctor on uninitialized profile (should show some failures or warnings)
echo "Running doctor on uninitialized profile..."
$COWEN_BIN doctor --profile "$TEST_PROFILE" > doctor_output.log || true
if grep -q "ERROR (App Secret Missing)" doctor_output.log; then
    echo "Correctly identified missing App Secret."
else
    echo -e "${RED}[FAILED]${NC} Doctor did not report missing App Secret."
    cat doctor_output.log
    exit 1
fi

# 3. Initialize and run doctor again
echo "Initializing profile and running doctor..."
$COWEN_BIN init --profile "$TEST_PROFILE" --app-mode self-built --app-key "k" --app-secret "s" --certificate "c" --encrypt-key "e" --stream-url "http://localhost:8080" > /dev/null

$COWEN_BIN doctor --profile "$TEST_PROFILE" > doctor_output_init.log
if ! grep -q "ERROR (App Secret Missing)" doctor_output_init.log; then
    echo "Correctly identified App Secret after init."
else
    echo -e "${RED}[FAILED]${NC} Doctor did not find App Secret after init."
    cat doctor_output_init.log
    exit 1
fi

if grep -q "OpenAPI" doctor_output_init.log; then
    echo "Network check included OpenAPI."
else
    echo -e "${RED}[FAILED]${NC} Doctor output missing network check."
    cat doctor_output_init.log
    exit 1
fi

echo -e "${GREEN}Test Case 44 PASSED!${NC}"
rm -f doctor_output.log doctor_output_init.log
