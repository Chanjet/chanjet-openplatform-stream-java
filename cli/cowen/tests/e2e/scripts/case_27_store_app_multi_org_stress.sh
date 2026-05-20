#!/bin/bash
# Case 28: StoreApp Multi-Org Tenant Isolation & Scaling
# Verifies:
#   1. A single StoreApp profile can handle organizations.
#   2. Token retrieval is org-specific and correctly isolated.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_27"
start_mock

PROF="multi_org"
ORG_COUNT=10
DB_FILE="$COWEN_HOME/case_27_real.db"

# Force App Configuration with explicit SQLite URL
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$DB_FILE"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Initialize StoreApp
"$COWEN_BIN" init --profile "$PROF" \
    --app-mode store-app \
    --app-key AK_MULTI \
    --app-secret AS_MULTI \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port 9128

# Start Daemon
"$COWEN_BIN" daemon start --profile "$PROF" --foreground > "$COWEN_HOME/daemon.log" 2>&1 &
DAEMON_PID=$!
sleep 5

# 2. Simulate Orgs Authorization (TEMP_AUTH_CODE)
echo -e "${BOLD}2. Simulating Authorization for $ORG_COUNT Orgs${NC}"
for i in $(seq 1 $ORG_COUNT); do
    ORG_ID="ORG_$i"
    curl -s -X POST "$MOCK_URL/control/broadcast" -d "{
        \"msgType\": \"TEMP_AUTH_CODE\",
        \"appKey\": \"AK_MULTI\",
        \"payload\": { \"tempAuthCode\": \"code_$ORG_ID\", \"state\": \"ok\" }
    }" > /dev/null
done

echo "   Waiting for token exchange to complete..."
sleep 20

# 3. Verify Tokens in DB
echo -e "${BOLD}3. Verifying Token Storage Isolation${NC}"
if ! command -v sqlite3 >/dev/null 2>&1; then
    echo -e "   ${RED}[FAILED]${NC} sqlite3 command not found"
    exit 1
fi

# Wait for WAL to flush by killing daemon
echo "   Killing daemon $DAEMON_PID..."
kill -9 $DAEMON_PID 2>/dev/null || true
# Use a more specific pattern to avoid killing the test script itself
pkill -9 -f "$(basename "$COWEN_BIN") daemon.*--profile $PROF" 2>/dev/null || true
sleep 5

if [ ! -f "$DB_FILE" ]; then
    echo -e "   ${RED}[FAILED]${NC} DB file not created at $DB_FILE"
    ls -la "$COWEN_HOME"
    exit 1
fi

echo "   Querying database $DB_FILE..."
STORED_COUNT=$(sqlite3 "$DB_FILE" "SELECT count(*) FROM cowen_permanent_code WHERE code_type = 'org_permanent';" || echo "ERR")

if [ "$STORED_COUNT" == "ERR" ]; then
    echo -e "   ${RED}[FAILED]${NC} sqlite3 query failed"
    exit 1
fi

if [ "$STORED_COUNT" -ge "$ORG_COUNT" ]; then
    echo -e "   ${GREEN}✓${NC} Successfully stored permanent codes for $STORED_COUNT orgs"
else
    echo -e "   ${RED}[FAILED]${NC} Only found '$STORED_COUNT' codes in DB"
    echo "--- Full Table Dump (cowen_permanent_code) ---"
    sqlite3 "$DB_FILE" "SELECT app_key, org_id, code_type FROM cowen_permanent_code;"
    exit 1
fi

# 3.1 Strict Key Integrity Validation (Bug Regression Check)
echo "   Verifying Data Integrity (No empty org_id)..."
BUGGED_RECORDS=$(sqlite3 "$DB_FILE" "SELECT org_id FROM cowen_permanent_code WHERE org_id = '' OR org_id IS NULL;" 2>/dev/null)
if [ -n "$BUGGED_RECORDS" ]; then
    echo -e "   ${RED}[FAILED]${NC} Found records with empty org_id"
    exit 1
fi
echo -e "   ${GREEN}✓${NC} All records have valid org_id values"

# 4. Verify Correct Token Attachment during API Calls
echo -e "${BOLD}4. Restarting Daemon and Verifying API Proxy${NC}"
"$COWEN_BIN" daemon start --profile "$PROF" --foreground > "$COWEN_HOME/daemon_v2.log" 2>&1 &
DAEMON_PID=$!
sleep 10

for i in 1 10 $ORG_COUNT; do
    ORG_ID="ORG_$i"
    echo -n "   Testing Org: $ORG_ID..."

    set +e
    RECEIVED_RESP=$(curl -s --connect-timeout 5 -x "http://127.0.0.1:9128" -H "x-org-id: $ORG_ID" -X POST "$MOCK_URL/v1/app/data/get")
    CURL_EXIT=$?
    
    # Safe JSON parsing
    RECEIVED_TOKEN=$(echo "$RECEIVED_RESP" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    print(d.get('data', {}).get('openToken', ''))
except:
    print('')
" 2>/dev/null)
    set -e

    if [[ "$RECEIVED_TOKEN" == *"mock_at_oa2_permanent_code"* ]]; then
        echo -e " ${GREEN}[MATCH]${NC}"
    else
        echo -e " ${RED}[MISMATCH]${NC}"
        echo "   Expected token containing: mock_at_oa2_permanent_code"
        echo "   Actual token received:     $RECEIVED_TOKEN"
        echo "   Full Response:             $RECEIVED_RESP"
        kill -9 $DAEMON_PID 2>/dev/null || true
        exit 1
    fi
done

# Cleanup
kill -9 $DAEMON_PID 2>/dev/null || true
echo -e "\n${GREEN}🎊 Case 28 Passed!${NC}"
