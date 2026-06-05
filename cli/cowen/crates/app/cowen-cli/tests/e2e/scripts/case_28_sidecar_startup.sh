#!/bin/bash
set -e
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_28"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. One-Liner Startup via Env Vars${NC}"

# Set Env Vars for Store-App sidecar
export COWEN_APP_MODE="store-app"
export COWEN_APP_KEY="AK_SIDECAR"
export COWEN_APP_SECRET="AS_SIDECAR"
export COWEN_ENCRYPT_KEY="1234567890123456"
export COWEN_WEBHOOK_TARGET="http://127.0.0.1:8080/cb"
export COWEN_OPENAPI_URL="$MOCK_URL"
export COWEN_STREAM_URL="$MOCK_WS"
export COWEN_PROXY_PORT=$(get_unused_port)

# Use a specific profile to ensure no collision
PROFILE="env-auto-init"

# Start daemon in background with --foreground to simulate container behavior
"$COWEN_BIN" --profile $PROFILE daemon start --foreground > "$COWEN_HOME/daemon.log" 2>&1 &
DAEMON_PID=$!

echo -e "  Waiting for auto-initialization..."
if wait_for_daemon "$PROFILE" 15; then
    echo -e "  ${GREEN}✓${NC} Profile '$PROFILE' verified via status"
else
    cat "$COWEN_HOME/daemon.log"
    kill $DAEMON_PID 2>/dev/null
    fail_suite "Profile '$PROFILE' NOT found or not running"
fi

# 2. Verify Daemon Status
"$COWEN_BIN" --profile $PROFILE status | grep -q "ACTIVE\|RUNNING"
assert_pass "Daemon is running from auto-init"

# 3. Verify Credentials Injected
"$COWEN_BIN" --profile $PROFILE status | grep -q "Security (Vault):"
assert_pass "Credentials verified in status"

# 4. Cleanup background process
kill $DAEMON_PID 2>/dev/null || true
sleep 1

echo -e "\n${BOLD}2. Global Store Override via Env Var${NC}"
# Create a fresh workspace for store test
setup_workspace "case_28_store_override"

# Override Store Type and DB URL
export COWEN_STORE_TYPE="innerdb"
DB_PATH="$COWEN_HOME/overridden.db"
export COWEN_DB_URL="innerdb://$DB_PATH"

# Run store status to verify detection
"$COWEN_BIN" store status > "$COWEN_HOME/store_status.out"
if grep -q "$DB_PATH" "$COWEN_HOME/store_status.out"; then
    echo -e "  ${GREEN}✓${NC} Store URL overridden via COWEN_DB_URL"
else
    cat "$COWEN_HOME/store_status.out"
    fail_suite "Store URL NOT overridden"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 28 Passed!${NC}"
