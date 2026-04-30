#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}🧪 Starting Multi-Profile Concurrency Exploratory Testing...${NC}"

# Setup isolated environment
export COWEN_HOME=$(pwd)/.cowen_multi_test
rm -rf "$COWEN_HOME"
mkdir -p "$COWEN_HOME"

BINARY_PATH="./target/debug/cowen"
MOCK_SERVER="./tests/mock_server.py"

# Start Mock Server in background
python3 "$MOCK_SERVER" &
MOCK_PID=$!

trap "kill $MOCK_PID || true" EXIT

# Wait for mock server
echo -n "   [WAIT] Waiting for Mock Server..."
for i in {1..5}; do
    if curl -s http://127.0.0.1:9099/v1/mock/spec > /dev/null; then
        echo -e " ${GREEN}[READY]${NC}"
        break
    fi
    sleep 1
done

# Step 1: Init Profile A (Self-Built)
PROF_A="prof-self-built"
echo -e "${YELLOW}Step 1: Initializing Profile A (Self-Built)...${NC}"
"$BINARY_PATH" init --profile "$PROF_A" \
    --app-mode self-built \
    --app-key AK_SELF \
    --app-secret AS_SELF \
    --encrypt-key 1234567890123456 \
    --certificate MOCK_CERT \
    --openapi-url http://127.0.0.1:9099 \
    --stream-url ws://127.0.0.1:9098 \
    --proxy-port 8081 \
    --webhook-target http://127.0.0.1:8081/webhook > /dev/null

# Step 2: Initializing Profile B (Store-App)
PROF_B="prof-store-app"
echo -e "${YELLOW}Step 2: Initializing Profile B (Store-App)...${NC}"
"$BINARY_PATH" init --profile "$PROF_B" \
    --app-mode store-app \
    --app-key AK_STORE \
    --app-secret AS_STORE \
    --encrypt-key 1234567890123456 \
    --openapi-url http://127.0.0.1:9099 \
    --stream-url ws://127.0.0.1:9098 \
    --proxy-port 8082 \
    --webhook-target http://127.0.0.1:8082/webhook > /dev/null

# Injecting initial token pair for Store-App (using multi-tenant key)
# Setting expiry to PAST to trigger immediate refresh
TOKEN_JSON='{"access_token":"init_token_b","refresh_token":"init_rt_b","expires_at":"2020-01-01T00:00:00Z","refresh_expires_at":"2099-01-01T00:00:00Z","created_at":"2020-01-01T00:00:00Z"}'
STORE_APP_KEY="oauth2_token_pair_user_AK_STORE_mock_org_mock_user"
sqlite3 "$COWEN_HOME/cowen.db" "INSERT INTO cowen_secret (profile, item_key, item_value) VALUES ('$PROF_B', '$STORE_APP_KEY', '$TOKEN_JSON');"

# Step 3: Starting Daemons for all profiles...
echo -e "${YELLOW}Step 3: Starting Daemons for all profiles...${NC}"
"$BINARY_PATH" daemon start --all > /dev/null
sleep 3
echo -e "   [OK] Daemons started."

# Step 4: Capture Initial Tokens (using sqlite3 because StoreApp auth token requires headers)
echo -e "${YELLOW}Step 4: Monitoring Concurrent Refresh...${NC}"
TOKEN_A_INIT=$(sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_token WHERE profile = 'primary' AND item_key = 'AK_SELF'")
TOKEN_B_INIT=$(sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_secret WHERE profile = '$PROF_B' AND item_key = '$STORE_APP_KEY'")

echo -e "   [INFO] Profile A Initial Token: $TOKEN_A_INIT"
echo -e "   [INFO] Profile B Initial Token: $TOKEN_B_INIT"

echo -n "   [WAIT] Waiting for refresh cycle (15s)..."
for i in {1..15}; do sleep 1; echo -n "."; done
echo -e " [DONE]"

# Step 5: Verify Refresh
TOKEN_A_NEW=$(sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_token WHERE profile = 'primary' AND item_key = 'AK_SELF'")
TOKEN_B_NEW=$(sqlite3 "$COWEN_HOME/cowen.db" "SELECT item_value FROM cowen_secret WHERE profile = '$PROF_B' AND item_key = '$STORE_APP_KEY'")

echo -e "   [INFO] Profile A Current Token: $TOKEN_A_NEW"
echo -e "   [INFO] Profile B Current Token: $TOKEN_B_NEW"

RESULT=0
if [ "$TOKEN_A_INIT" != "$TOKEN_A_NEW" ]; then
    echo -e "${GREEN}   [OK] Profile A (Self-Built) Refreshed.${NC}"
else
    # Maybe it hasn't refreshed yet because it waits for AppTicket?
    # Actually SelfBuilt Renewer triggers push.
    echo -e "${RED}   [FAIL] Profile A (Self-Built) FAILED to refresh.${NC}"
    RESULT=1
fi

if [ "$TOKEN_B_INIT" != "$TOKEN_B_NEW" ]; then
    echo -e "${GREEN}   [OK] Profile B (Store-App) Refreshed.${NC}"
else
    echo -e "${RED}   [FAIL] Profile B (Store-App) FAILED to refresh.${NC}"
    RESULT=1
fi

# Step 6: Check logs for isolation
if grep -q "prof-self-built" "$COWEN_HOME/logs/prof-self-built_sys.log" && grep -q "prof-store-app" "$COWEN_HOME/logs/prof-store-app_sys.log"; then
    echo -e "${GREEN}   [OK] Log isolation verified.${NC}"
else
    echo -e "${YELLOW}   [WARN] Logs might be named differently.${NC}"
fi

if [ $RESULT -eq 0 ]; then
    echo -e "\n${GREEN}🎉 Multi-Profile Concurrency Test PASSED!${NC}"
else
    echo -e "\n${RED}❌ Multi-Profile Concurrency Test FAILED!${NC}"
fi

exit $RESULT
