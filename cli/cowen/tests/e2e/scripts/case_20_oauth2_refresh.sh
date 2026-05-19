#!/bin/bash
set -e
# Case 20: OAuth2 Refresh Token Renewal (Log-Driven Recovery)

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
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
# Use --no-ai and --no-telemetry to speed up and simplify logs
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode oauth2 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT \
    --no-ai \
    --no-telemetry > "$COWEN_HOME/init.log" 2>&1 &
INIT_PID=$!

echo "   Init PID: $INIT_PID (waiting for browser link in log)"

# 2. Extract State and Port from Log
STATE=""
PORT=""
for i in {1..30}; do
    if [ -f "$COWEN_HOME/init.log" ]; then
        # Try to find the URL and extract parameters
        LINK=$(grep -o "https://market.chanjet.com/user/v2/authorize?.*" "$COWEN_HOME/init.log" | head -n 1)
        if [[ -n "$LINK" ]]; then
            # Extract state and port using Python for URL-safe regex
            STATE=$(echo "$LINK" | python3 -c "import sys,re; m=re.search(r'state=([^&]+)', sys.stdin.read()); print(m.group(1) if m else '')")
            PORT=$(echo "$LINK" | python3 -c "import sys,re; m=re.search(r'127\.0\.0\.1%3A(\d+)', sys.stdin.read()); print(m.group(1) if m else '')")
            
            if [[ -n "$STATE" ]] && [[ -n "$PORT" ]]; then
                echo -e "   ${GREEN}[EXTRACTED]${NC} Port: $PORT, State: ${STATE:0:8}..."
                break
            fi
        fi
    fi
    echo -n "."
    sleep 1
done

if [[ -z "$STATE" ]]; then
    echo -e "   ${RED}[FAILED]${NC} Failed to extract OAuth2 context from logs"
    cat "$COWEN_HOME/init.log"
    kill -9 "$INIT_PID" 2>/dev/null || true
    exit 1
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
    echo -e "   ${RED}[FAILED]${NC} Token retrieval failed"
    exit 1
fi

# 5. Wait for expiration and trigger refresh
echo -e "${BOLD}3. Wait for Expiration (8s) and Trigger Refresh${NC}"
sleep 8

TOKEN_2=$(extract_token "$PROF")
echo "     New Token: $TOKEN_2"

if [[ -n "$TOKEN_2" ]] && [[ "$TOKEN_2" == mock_at_oa2_* ]] && [[ "$TOKEN_2" != "$TOKEN_1" ]]; then
    echo -e "   ${GREEN}✓${NC} Token successfully renewed"
else
    echo -e "   ${RED}[FAILED]${NC} Token refresh failed or token did not change. Got: $TOKEN_2"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 20 Passed!${NC}"
