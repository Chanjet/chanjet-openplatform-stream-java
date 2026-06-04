#!/bin/bash
# Test Case 70: Init Default App Mode
# Verify that running cowen init without --app-mode defaults to oauth2.

set -e

# Source common utilities
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BLUE}Starting Test Case 70: Init Default App Mode${NC}"

# 1. Setup clean environment for this test
setup_workspace "case_70"
TEST_PROFILE="test_default_oauth2"

cleanup() {
    pkill -f "$TEST_PROFILE" || true
    cleanup_suite
}

# trap cleanup EXIT

echo -e "${YELLOW}Scenario 1: Init without specifying --app-mode${NC}"
# Run in background because oauth2 mode blocks on login
"$COWEN_BIN" init --profile "$TEST_PROFILE" --app-key "dummy_key" --app-secret "dummy_secret" --certificate "dummy_cert" --encrypt-key "dummy_ek" > "$COWEN_HOME/oauth_init.log" 2>&1 &
INIT_PID=$!

# Wait for it to create the profile in DB
max_retries=10
retry_count=0
success=false

while [ $retry_count -lt $max_retries ]; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        if sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_config WHERE profile='$TEST_PROFILE';" | grep -q "oauth2"; then
            echo -e " ${GREEN}[OK]${NC} Default app mode is correctly set to oauth2 in database."
            success=true
            break
        fi
    fi
    sleep 1
    retry_count=$((retry_count + 1))
done

if [ "$success" = false ]; then
    echo -e " ${RED}[FAIL]${NC} app_mode is not oauth2 in database after timeout!"
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_config WHERE profile='$TEST_PROFILE';" || true
    fi
    cat "$COWEN_HOME/oauth_init.log" || true
    exit 1
fi

echo "   Simulating Ctrl+C on PID $INIT_PID..."
kill -TERM $INIT_PID || true
wait $INIT_PID || true

echo -e "${GREEN}Test Case 70 PASSED!${NC}"
