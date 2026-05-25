#!/bin/bash
set -e

source "$(dirname "$0")/common.sh"
setup_workspace "case_64_profile_rename_oauth2"
start_mock

PROF="p1"

echo -e "${BOLD}1. Initialize and Login with OAuth2${NC}"
PROXY_PORT=$((MOCK_PORT + 110))

# Start init in background
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port $PROXY_PORT > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

echo -n "   Waiting for auth session..."
SESSION_JSON=""
for i in {1..40}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
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

echo -n "   Simulating browser callback to port ${REDIRECT_PORT}..."
MAX_RETRIES=40
SUCCESS=0
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s -f "http://127.0.0.1:${REDIRECT_PORT}/callback?code=mock_code&state=${STATE}" > /dev/null 2>&1; then
        SUCCESS=1
        break
    fi
    sleep 0.5
done

if [ $SUCCESS -eq 0 ]; then
    kill "$INIT_PID" 2>/dev/null
    fail_suite "- Redirect port unreachable after $MAX_RETRIES attempts"
fi
echo -e " ${GREEN}[OK]${NC}"
wait "$INIT_PID"

echo -e "\n${BOLD}2. Verify Initial Login Status${NC}"
OUT=$("$COWEN_BIN" status --profile "$PROF")
if echo "$OUT" | grep -q "Not logged in or session expired"; then
    fail_suite "Initial login failed"
fi
echo -e "  ${GREEN}✓${NC} $PROF is logged in successfully"

echo -e "\n${BOLD}3. Rename Profile${NC}"
"$COWEN_BIN" profile rename "$PROF" p2 >/dev/null
echo -e "  ${GREEN}✓${NC} Renamed $PROF to p2"

echo -e "\n${BOLD}4. Verify Status After Rename${NC}"
OUT2=$("$COWEN_BIN" status --profile p2)
if echo "$OUT2" | grep -q "Not logged in or session expired"; then
    echo "$OUT2"
    fail_suite "Authentication lost after rename! cowen_tenant_token migration failed."
fi
echo -e "  ${GREEN}✓${NC} p2 retains login status successfully"

echo -e "\n${GREEN}🎊 Case 64 Passed! OAuth2 token retained across rename.${NC}"
