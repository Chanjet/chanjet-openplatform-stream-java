#!/bin/bash
set -e
# Case 20: OAuth2 Refresh Token Renewal (Log-Driven Recovery)

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_20"
start_mock

PROF="oa2_refresh"

# Calculate dynamic ports based on MOCK_PORT to avoid parallel collisions
PROXY_PORT=$((MOCK_PORT + 101))

# Initialize OAuth2 in background
# Use --no-telemetry to speed up and simplify logs
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT \
    --no-telemetry > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

echo "   Init PID: $INIT_PID (waiting for browser link in log)"

# 2. Extract State and Port from SQLite DB
SESSION_JSON=""
for i in {1..40}; do
    if [ -f "$COWEN_HOME/cowen.db" ]; then
        SESSION_JSON=$( (sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;") 2>/dev/null || echo "")
        [ -n "$SESSION_JSON" ] && break
    fi
    echo -n "."
    sleep 0.5
done

if [[ -z "$SESSION_JSON" ]]; then
    cat "$COWEN_HOME/init.log"
    kill -9 "$INIT_PID" 2>/dev/null || true
    fail_suite "Failed to extract OAuth2 context from database"
fi

PORT=$(get_json_field "$SESSION_JSON" "redirect_port")
STATE=$(get_json_field "$SESSION_JSON" "state")
echo -e "   ${GREEN}[EXTRACTED]${NC} Port: $PORT, State: ${STATE:0:8}..."

# Assert that the browser mock was called correctly
echo -n "   Verifying browser trigger..."
if grep -q "Browser mock triggered for URL" "$COWEN_HOME/init.log" 2>/dev/null; then
    echo -e " ${GREEN}[OK]${NC}"
else
    kill "$INIT_PID" 2>/dev/null
    fail_suite "[FAIL - Browser open command was not triggered]"
fi

# 3. Simulate Callback
# Set mock server to return tokens that expire in 7 seconds
curl -s -X POST "$MOCK_URL/control/config" -d '{"token_expires_in": 7}' > /dev/null
echo "   Triggering callback on port $PORT..."
sleep 1
curl -s "http://127.0.0.1:${PORT}/callback?code=mock_code&state=${STATE}" > /dev/null

# Wait for init process to finish
for i in {1..15}; do
    if ! kill -0 "$INIT_PID" 2>/dev/null; then
        echo -e "   ${GREEN}[DONE]${NC} Init process finalized"
        break
    fi
    sleep 1
done

# 4. Get Initial Token
echo -e "${BOLD}2. Get Initial Token${NC}"
TOKEN_1=$(extract_token "$PROF")
echo "     Initial Token: $TOKEN_1"

if [[ -n "$TOKEN_1" ]] && [[ "$TOKEN_1" == mock_at_oa2_* ]]; then
    echo -e "   ${GREEN}✓${NC} Initial token obtained"
else
    fail_suite "Token retrieval failed"
fi

# 5. Wait for expiration and trigger refresh
echo -e "${BOLD}3. Wait for Expiration (8s) and Trigger Refresh${NC}"
sleep 8

TOKEN_2=$(extract_token "$PROF")
echo "     New Token: $TOKEN_2"

if [[ -n "$TOKEN_2" ]] && [[ "$TOKEN_2" == mock_at_oa2_* ]] && [[ "$TOKEN_2" != "$TOKEN_1" ]]; then
    echo -e "   ${GREEN}✓${NC} Token successfully renewed"
else
    fail_suite "Token refresh failed or token did not change. Got: $TOKEN_2"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 20 Passed!${NC}"
