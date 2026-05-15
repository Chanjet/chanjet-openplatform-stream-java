#!/bin/bash
# Case 44: Auth Logout and Re-Login Flow (OAuth2)
#
# Verifies:
#   1. 'auth logout' clears all tokens.
#   2. 'status' reports WARN / "Not logged in" after logout.
#   3. 'auth login' automatically falls back to browser flow when tokens are missing.

source tests/e2e/scripts/common.sh

setup_workspace "auth_logout_login"
trap cleanup_suite EXIT
start_mock

PROF="logout_test"

# ============================================================
# Phase 1: Initial Login (Mocked PKCE)
# ============================================================
echo -e "${BOLD}1. Initial OAuth2 Init${NC}"

export COWEN_SKIP_BROWSER=true

# Calculate dynamic ports based on MOCK_PORT to avoid parallel collisions
PROXY_PORT=$((MOCK_PORT + 100))

# Start init in background
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT > /dev/null 2>&1 &
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
[ -z "$SESSION_JSON" ] && { echo "Timeout"; exit 1; }
echo -e " ${GREEN}[FOUND]${NC}"

REDIRECT_PORT=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['redirect_port'])")
STATE=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['state'])")

# Callback
curl -s "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_code&state=${STATE}" > /dev/null
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
    echo "$OUT"
    exit 1
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
    echo -e " ${RED}[FAILED]${NC} - No re-auth flow triggered"
    kill "$LOGIN_PID" 2>/dev/null
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 44 Passed!${NC}"
