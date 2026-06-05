#!/bin/bash
set -e
# Case 03: OAuth2 Full-Lifecycle E2E Test
#
# Architecture:
#   OAuth2 init is BLOCKING - it spawns a finalizer process that listens
#   on a local callback port, then waits for oauth2_token_pair in vault.
#
# Test Strategy:
#   1. Run `cowen init --app-mode oauth2` in BACKGROUND (because it blocks)
#   2. Poll SQLite for `pending_auth_session` to extract redirect_port & state
#   3. Simulate browser callback: curl → http://127.0.0.1:{port}/callback?code=mock&state={state}
#   4. The finalizer receives the code, calls mock /oauth2/token, saves token pair
#   5. The blocked init detects token pair and returns
#   6. Verify token was acquired correctly
#   7. Test daemon auto-refresh with expired token injection

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)

setup_workspace "case_03"
trap cleanup_suite EXIT
start_mock

PROF="oa2"

# ============================================================
# Phase 1: OAuth2 Init (Full PKCE Authorization Flow)
# ============================================================

echo -e "${BOLD}1. OAuth2 Init (PKCE Flow)${NC}"

# Launch init in background (it blocks waiting for browser callback)
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT \
    --webhook-target "http://127.0.0.1:8080/cb" > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

echo "   Init PID: $INIT_PID (blocking, waiting for browser callback)"

# Poll DB for pending_auth_session (the session is written before browser opens)
echo -n "   Waiting for auth session..."
SESSION_JSON=""
for i in {1..30}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        # In v0.3.0, sessions are stored in cowen_token with 'global' profile and 'session:' prefix
        SESSION_JSON=$(sqlite3 "$COWEN_HOME/cowen.db" \
            "SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;" 2>/dev/null)
        if [ -n "$SESSION_JSON" ]; then
            echo -e " ${GREEN}[FOUND]${NC}"
            break
        fi
    fi
    echo -n "."
    sleep 0.5
done

if [ -z "$SESSION_JSON" ]; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[TIMEOUT]"
fi

# Extract redirect_port and state from session JSON
REDIRECT_PORT=$(get_json_field "$SESSION_JSON" "redirect_port")
STATE=$(get_json_field "$SESSION_JSON" "state")
echo "   Extracted: port=$REDIRECT_PORT, state=${STATE:0:8}..."

# Wait a moment for the finalizer listener to be ready
sleep 2

# Assert that the browser mock was called correctly
echo -n "   Verifying browser trigger..."
if grep -q "Browser mock triggered for URL" "$COWEN_HOME/init.log" 2>/dev/null || grep -q "Browser mock triggered for URL" "$COWEN_HOME/logs/daemon.stdout.log" 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[FAIL - Browser open command was not triggered]"
fi

# Simulate browser callback (this is what the platform redirect would do)
echo -n "   Simulating browser callback..."
CALLBACK_RESP=$(curl -s -o /dev/null -w "%{http_code}" \
    "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_auth_code_12345&state=${STATE}")
if [ "$CALLBACK_RESP" == "200" ]; then
    echo -e " ${GREEN}[OK]${NC}"
else
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[HTTP $CALLBACK_RESP]"
fi

# Wait for init to complete (it should unblock now)
echo -n "   Waiting for init to complete..."
for i in {1..15}; do
    if ! kill -0 "$INIT_PID" 2>/dev/null; then
        echo -e " ${GREEN}[DONE]${NC}"
        break
    fi
    sleep 1
done

if kill -0 "$INIT_PID" 2>/dev/null; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[TIMEOUT - init still blocking]"
fi

# Verify token was acquired
T=$(extract_token "$PROF")
if [ -n "$T" ] && [[ "$T" == mock_at_oa2* ]]; then
    echo -e "  ${GREEN}✓${NC} Initial token acquired via PKCE flow: $T"
else
    fail_suite "Token acquisition failed after PKCE flow"
fi

# ============================================================
# Phase 2: Daemon Auto-Refresh (Token Lifecycle)
# ============================================================

echo -e "\n${BOLD}2. Daemon Auto-Refresh (Expired Token Rotation)${NC}"

# Inject expired token pair to simulate token expiry
TOKEN_PAIR='{"access_token":"expired_at","refresh_token":"old_rt","expires_at":"2020-01-01T00:00:00Z","refresh_expires_at":"2099-01-01T00:00:00Z","created_at":"2020-01-01T00:00:00Z"}'
sqlite3 "$COWEN_HOME/cowen.db" \
    "UPDATE cowen_secret SET item_value='$TOKEN_PAIR' WHERE profile='$PROF' AND item_key='oauth2_token_pair';"
assert_pass "Expired token injected"

# Start daemon (it should detect the expired token and refresh it)
"$COWEN_BIN" daemon start --profile "$PROF" >/dev/null
echo -n "  Waiting for background refresh..."
T=$(wait_for_token "$PROF" "mock_at_oa2" 30)
if [ -n "$T" ]; then
    echo -e " ${GREEN}[OK]${NC}"
    assert_sanitized "$T" "Refreshed Token sanitization"
    echo "   Refreshed Token: $T"
else
    fail_suite "[FAIL]"
fi

echo -e "\n${GREEN}🎊 Case 03 Passed!${NC}"
