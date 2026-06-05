#!/bin/bash
# Test Case 37: Initialization Failure Cleanup
# Verify that temporary profiles are removed when init fails or is cancelled across all modes.

set -e
# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BLUE}Starting Test Case 37: Initialization Failure Cleanup${NC}"

# 1. Setup clean environment for this test
setup_workspace "case_37"
TEST_PROFILE="cleanup_test_$(date +%s)"

# Use a sub-directory in COWEN_HOME for this specific test
REAL_COWEN_HOME="$COWEN_HOME"
export COWEN_HOME="$REAL_COWEN_HOME/cleanup_sandbox"
rm -rf "$COWEN_HOME"
mkdir -p "$COWEN_HOME"

cleanup() {
    # Ensure any remaining cowen processes started by this script are killed
    pkill -f "$TEST_PROFILE" || true
    cleanup_suite
}
# trap cleanup EXIT

check_profile_absent() {
    local prof=$1
    if "$COWEN_BIN" profile list | grep -q "$prof"; then
        fail_suite "Profile '$prof' still exists in list."
    else
        echo -e " ${GREEN}[OK]${NC} Profile '$prof' is absent."
    fi
    
    if [ -f "$COWEN_HOME/${prof}.yaml" ]; then
        fail_suite "Profile file '$COWEN_HOME/${prof}.yaml' still exists."
    else
        echo -e " ${GREEN}[OK]${NC} Profile file is physically removed."
    fi
}

# --- Scenario 1: Self-Built Missing Params ---
echo -e "${YELLOW}Scenario 1: Self-Built Mode with missing credentials${NC}"
"$COWEN_BIN" init --profile "${TEST_PROFILE}_sb" --app-mode self-built --app-key "some-key" || true
check_profile_absent "${TEST_PROFILE}_sb"

# --- Scenario 2: Store-App Missing Params ---
echo -e "${YELLOW}Scenario 2: Store-App Mode with missing credentials${NC}"
"$COWEN_BIN" init --profile "${TEST_PROFILE}_store" --app-mode store-app --app-key "some-key" || true
check_profile_absent "${TEST_PROFILE}_store"

# --- Scenario 3: OAuth2 Cancellation (Ctrl+C Simulation) ---
echo -e "${YELLOW}Scenario 3: OAuth2 Mode Cancellation (Ctrl+C Simulation)${NC}"
# Use a background process and kill it to simulate user interrupt
"$COWEN_BIN" init --profile "${TEST_PROFILE}_oauth" --app-mode oauth2 > "$COWEN_HOME/oauth_init.log" 2>&1 &
INIT_PID=$!

# Wait for it to create the profile and start listening
sleep 3
echo "   Simulating Ctrl+C on PID $INIT_PID..."
kill -TERM $INIT_PID || true
wait $INIT_PID || true
sleep 1

check_profile_absent "${TEST_PROFILE}_oauth"

# --- Scenario 4: Preservation of EXISTING Profiles ---
echo -e "${YELLOW}Scenario 4: Ensure EXISTING profiles are NOT deleted on failure${NC}"
# Create a valid profile first
"$COWEN_BIN" init --profile "${TEST_PROFILE}_existing" --app-mode self-built --app-key "K" --app-secret "S" --certificate "C" --encrypt-key "E" > /dev/null
echo "   Created valid profile '${TEST_PROFILE}_existing'."

# Now attempt to "re-init" it with failing params
"$COWEN_BIN" init --profile "${TEST_PROFILE}_existing" --app-mode self-built --app-key "ONLY_KEY" || true

if "$COWEN_BIN" profile list | grep -q "${TEST_PROFILE}_existing"; then
    echo -e " ${GREEN}[OK]${NC} Existing profile was preserved."
else
    fail_suite "Existing profile was incorrectly deleted!"
fi

echo -e "${GREEN}Test Case 37 PASSED!${NC}"
