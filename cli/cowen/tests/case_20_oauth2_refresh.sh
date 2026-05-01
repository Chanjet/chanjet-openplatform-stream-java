#!/bin/bash
# Case 20: OAuth2 Refresh Token Renewal
# Verifies:
#   1. Initial token retrieval via authorization_code (Simulated).
#   2. Token renewal via refresh_token when access_token expires.

source tests/common.sh

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_20"
start_mock

PROF="oa2_refresh"
DB_FILE="$COWEN_HOME/case_20_isolated.db"

# FORCE ISOLATION: Override storage to use an explicit, unique DB file
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$DB_FILE"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Initialize OAuth2 in background (Wait for callback)
# We use a unique proxy port to avoid parallel collision
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --app-key AK_OAUTH \
    --app-secret AS_OAUTH \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port 9101 > /dev/null 2>&1 &
INIT_PID=$!

echo "   Init PID: $INIT_PID (waiting for callback)"

# Poll DB for session with high tolerance for parallel I/O lag
echo -n "   Waiting for auth session..."
SESSION_JSON=""
for i in {1..60}; do
    if [ -f "$DB_FILE" ]; then
        # Check if the table exists first to avoid error noise
        if sqlite3 "$DB_FILE" "SELECT name FROM sqlite_master WHERE type='table' AND name='cowen_config';" | grep -q "cowen_config"; then
            SESSION_JSON=$(sqlite3 -line -cmd ".timeout 10000" "$DB_FILE" \
                "SELECT item_value FROM cowen_config WHERE profile='$PROF' AND item_key='pending_auth_session' LIMIT 1;" 2>/dev/null)
            if [ -n "$SESSION_JSON" ]; then
                echo -e " ${GREEN}[FOUND]${NC}"
                break
            fi
        fi
    fi
    echo -n "."
    sleep 1
done

if [ -z "$SESSION_JSON" ]; then
    echo -e "   ${RED}[FAILED]${NC} Auth session not created or DB locked"
    [ -f "$DB_FILE" ] && echo "     DB exists but session missing."
    ls -l "$COWEN_HOME"
    kill -9 "$INIT_PID" 2>/dev/null
    exit 1
fi

PORT=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['redirect_port'])")
STATE=$(echo "$SESSION_JSON" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['state'])")

# Simulate Callback
# Set mock server to return tokens that expire in 5 seconds
curl -s -X POST "$MOCK_URL/control/config" -d '{"token_expires_in": 5}' > /dev/null
curl -s "http://127.0.0.1:${PORT}/callback?code=mock_code&state=${STATE}" > /dev/null

# Wait for init to unblock
for i in {1..20}; do
    if ! kill -0 "$INIT_PID" 2>/dev/null; then
        break
    fi
    sleep 1
done

# 2. Get Initial Token
echo -e "${BOLD}2. Get Initial Token${NC}"
TOKEN_1=$(extract_token "$PROF")
echo "     Initial Token: $TOKEN_1"

if [[ "$TOKEN_1" == *"authorization_code"* ]]; then
    echo -e "   ${GREEN}✓${NC} Initial token obtained via authorization_code"
else
    echo -e "   ${RED}[FAILED]${NC} Initial token type incorrect or retrieval failed"
    # Try to check if token exists at all
    sqlite3 -line -cmd ".timeout 5000" "$DB_FILE" "SELECT * FROM cowen_token;"
    exit 1
fi

# 3. Wait for expiration and trigger refresh
echo -e "${BOLD}3. Wait for Expiration (8s) and Trigger Refresh${NC}"
sleep 8

# Requesting a new token should now trigger refresh_token flow
TOKEN_2=$(extract_token "$PROF")
echo "     New Token: $TOKEN_2"

if [[ "$TOKEN_2" == *"refresh_token"* ]]; then
    echo -e "   ${GREEN}✓${NC} Token successfully renewed via refresh_token"
elif [[ "$TOKEN_2" == "$TOKEN_1" ]]; then
    echo -e "   ${RED}[FAILED]${NC} Token was NOT refreshed (still using old token)"
    exit 1
else
    echo -e "   ${RED}[FAILED]${NC} Unexpected token state: $TOKEN_2"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 20 Passed!${NC}"
