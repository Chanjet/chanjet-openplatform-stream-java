#!/bin/bash
# Test Case 47: Cowen Doctor
# Purpose: Verify that the cowen doctor command correctly diagnoses the environment.

set -e
NC='\033[0m'
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'

echo -e "${BLUE}Starting Test Case 47: Cowen Doctor${NC}"

# 1. Setup environment
TEST_ID=$RANDOM
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
mkdir -p "$TEST_BASE"

# 🚀 Isolate binary for process manager visibility
# Use SOURCE_BIN if set (from run_parallel), otherwise default
BASE_BIN="${SOURCE_BIN:-$(pwd)/target/release/cowen}"
cp "$BASE_BIN" "$TEST_BASE/cowen_case_47"
export COWEN_BIN="$TEST_BASE/cowen_case_47"
chmod +x "$COWEN_BIN"

export COWEN_HOME="$TEST_BASE/case_47_$TEST_ID"
mkdir -p "$COWEN_HOME"
TEST_PROFILE="doctor_test_$TEST_ID"

# 2. Run doctor on uninitialized profile (should show some failures or warnings)
echo "Running doctor on uninitialized profile..."
$COWEN_BIN doctor --profile "$TEST_PROFILE" > doctor_output.log || true
if grep -q "App Secret:    MISSING" doctor_output.log; then
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
if grep -q "App Secret:    FOUND" doctor_output_init.log; then
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

echo -e "${GREEN}Test Case 47 PASSED!${NC}"
rm -rf "$COWEN_HOME" doctor_output.log doctor_output_init.log
