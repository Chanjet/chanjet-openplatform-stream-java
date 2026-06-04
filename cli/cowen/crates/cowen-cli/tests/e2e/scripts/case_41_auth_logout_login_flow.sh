#!/bin/bash
set -e
# Case 41: Auth Logout and Re-Login Flow (OAuth2)
#
# Verifies:
#   1. 'auth logout' clears all tokens.
#   2. 'status' reports WARN / "Not logged in" after logout.
#   3. 'auth login' automatically falls back to browser flow when tokens are missing.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_41"
trap cleanup_suite EXIT
start_mock

PROF="logout_test"

# ============================================================
# Phase 1: Initial Login (Mocked PKCE)
# ============================================================
echo -e "${BOLD}1. Initial OAuth2 Init${NC}"

# Calculate dynamic ports based on MOCK_PORT to avoid parallel collisions
PROXY_PORT=$((MOCK_PORT + 100))

# Start init in background
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

# Extract state/port from DB
echo -n "   Waiting for auth session..."
SESSION_JSON=""
for i in {1..40}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        # Use a subshell and hide errors to handle 'table not found' during early init
        SESSION_JSON=$( (sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;") 2>/dev/null || echo "")
        [ -n "$SESSION_JSON" ] && break
    fi
    sleep 0.5
done
[ -z "$SESSION_JSON" ] && fail_suite "Timeout waiting for auth session"
echo -e " ${GREEN}[FOUND]${NC}"

# Assert that the browser mock was called correctly
echo -n "   Verifying browser trigger..."
if grep -q "Browser mock triggered for URL" "$COWEN_HOME/init.log" 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[FAIL - Browser open command was not triggered]"
fi

REDIRECT_PORT=$(get_json_field "$SESSION_JSON" "redirect_port")
STATE=$(get_json_field "$SESSION_JSON" "state")

# Wait/Retry Callback to avoid port binding race condition
echo -n "   Simulating browser callback to port ${REDIRECT_PORT}..."
MAX_RETRIES=40
SUCCESS=0
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s -f "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_code&state=${STATE}" > /dev/null; then
        SUCCESS=1
        break
    fi
    echo -n "."
    sleep 0.5
done

if [ $SUCCESS -eq 0 ]; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "- Redirect port  unreachable after $MAX_RETRIES attempts"
fi
echo -e " ${GREEN}[OK]${NC}"
wait "$INIT_PID"

# Verify initial login success
T=$("$COWEN_BIN" auth token --profile "$PROF" --format text)
assert_match "$T" "mock_at_oa2" "Initial token acquired"

# ============================================================
# Phase 2: Logout and Status Verification
# ============================================================
echo -e "\n${BOLD}2. Logout and Status Verification${NC}"

"$COWEN_BIN" auth logout --profile "$PROF" > /dev/null
assert_pass "Logout executed"

echo -n "   Verifying status reports 'Not logged in'..."
OUT=$("$COWEN_BIN" status --profile "$PROF")
if echo "$OUT" | grep -q "Not logged in or session expired"; then
    echo -e " ${GREEN}[OK]${NC}"
else
    echo -e " ${RED}[FAILED]${NC}"
    fail_suite "$OUT"
fi

# ============================================================
# Phase 3: Re-Login Fallback
# ============================================================
echo -e "\n${BOLD}3. Re-Login Fallback to Browser Flow${NC}"

# Trigger auth login - it should notice missing token and start a new PKCE flow
"$COWEN_BIN" auth login --profile "$PROF" > /dev/null 2>&1 &
LOGIN_PID=$!

echo -n "   Waiting for new re-auth session..."
REAUTH_SESSION=""
for i in {1..30}; do
    # Check if a new session is created in DB
    REAUTH_SESSION=$(sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;" 2>/dev/null)
    [ -n "$REAUTH_SESSION" ] && break
    sleep 0.5
done

if [ -n "$REAUTH_SESSION" ]; then
    echo -e " ${GREEN}[FOUND]${NC}"
    # Cleanup re-auth process as we've verified it triggered the flow
    kill "$LOGIN_PID" 2>/dev/null
else
    kill "$LOGIN_PID" 2>/dev/null
    fail_suite "- No re-auth flow triggered"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 41 Passed!${NC}"
