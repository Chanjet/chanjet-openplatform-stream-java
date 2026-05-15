#!/bin/bash
# Test Case 40: Initialization Failure Cleanup
# Verify that temporary profiles are removed when init fails or is cancelled across all modes.

set -e
NC='\033[0m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'

# COWEN_BIN is inherited from common.sh/environment
if [ ! -f "$COWEN_BIN" ]; then
    COWEN_BIN="../../bin/macos-aarch64/cowen"
fi

echo -e "${BLUE}Starting Test Case 40: Initialization Failure Cleanup${NC}"

# 1. Setup clean environment for this test
TEST_PROFILE="cleanup_test_$(date +%s)"
export COWEN_HOME="/tmp/cowen_cleanup_test"
rm -rf "$COWEN_HOME"
mkdir -p "$COWEN_HOME"

check_profile_absent() {
    local prof=$1
    if $COWEN_BIN profile list | grep -q "$prof"; then
        echo -e " ${RED}[FAILED]${NC} Profile '$prof' still exists in list."
        exit 1
    else
        echo -e " ${GREEN}[OK]${NC} Profile '$prof' is absent."
    fi
    
    if [ -f "$COWEN_HOME/${prof}.yaml" ]; then
        echo -e " ${RED}[FAILED]${NC} Profile file '$COWEN_HOME/${prof}.yaml' still exists."
        exit 1
    else
        echo -e " ${GREEN}[OK]${NC} Profile file is physically removed."
    fi
}

# --- Scenario 1: Self-Built Missing Params ---
echo -e "${YELLOW}Scenario 1: Self-Built Mode with missing credentials${NC}"
# Purpose: Should fail validation and delete the new profile
$COWEN_BIN init --profile "${TEST_PROFILE}_sb" --app-mode self-built --app-key "some-key" || true
check_profile_absent "${TEST_PROFILE}_sb"

# --- Scenario 2: Store-App Missing Params ---
echo -e "${YELLOW}Scenario 2: Store-App Mode with missing credentials${NC}"
$COWEN_BIN init --profile "${TEST_PROFILE}_store" --app-mode store-app --app-key "some-key" || true
check_profile_absent "${TEST_PROFILE}_store"

# --- Scenario 3: OAuth2 Cancellation (Ctrl+C Simulation) ---
echo -e "${YELLOW}Scenario 3: OAuth2 Mode Cancellation (Ctrl+C Simulation)${NC}"
# Use a background process and kill it to simulate user interrupt
$COWEN_BIN init --profile "${TEST_PROFILE}_oauth" --app-mode oauth2 > /tmp/oauth_init.log 2>&1 &
INIT_PID=$!

# Wait for it to create the profile and start listening
sleep 2
echo "   Simulating Ctrl+C on PID $INIT_PID..."
kill -INT $INIT_PID || true
sleep 1

check_profile_absent "${TEST_PROFILE}_oauth"

# --- Scenario 4: Preservation of EXISTING Profiles ---
echo -e "${YELLOW}Scenario 4: Ensure EXISTING profiles are NOT deleted on failure${NC}"
# Create a valid profile first
$COWEN_BIN init --profile "${TEST_PROFILE}_existing" --app-mode self-built --app-key "K" --app-secret "S" --certificate "C" --encrypt-key "E" > /dev/null
echo "   Created valid profile '${TEST_PROFILE}_existing'."

# Now attempt to "re-init" it with failing params
$COWEN_BIN init --profile "${TEST_PROFILE}_existing" --app-mode self-built --app-key "ONLY_KEY" || true

if $COWEN_BIN profile list | grep -q "${TEST_PROFILE}_existing"; then
    echo -e " ${GREEN}[OK]${NC} Existing profile was preserved."
else
    echo -e " ${RED}[FAILED]${NC} Existing profile was incorrectly deleted!"
    exit 1
fi

echo -e "${GREEN}Test Case 40 PASSED!${NC}"
rm -rf "$COWEN_HOME"
