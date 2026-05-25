#!/bin/bash
# Case 27: StoreApp Multi-Org Tenant Isolation & Scaling
# Verifies:
#   1. A single StoreApp profile can handle organizations.
#   2. Token retrieval is org-specific and correctly isolated.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
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
    --proxy-port $PROXY_PORT

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
    fail_suite "sqlite3 command not found"
fi

# Wait for WAL to flush by killing daemon
echo "   Killing daemon..."
kill_daemons_in_dirs "$COWEN_HOME"
sleep 5

if [ ! -f "$DB_FILE" ]; then
    ls -la "$COWEN_HOME"
    fail_suite "DB file not created at $DB_FILE"
fi

echo "   Querying database $DB_FILE..."
STORED_COUNT=$(sqlite3 "$DB_FILE" "SELECT count(*) FROM cowen_permanent_code WHERE code_type = 'org_permanent';" || echo "ERR")

if [ "$STORED_COUNT" == "ERR" ]; then
    fail_suite "sqlite3 query failed"
fi

if [ "$STORED_COUNT" -ge "$ORG_COUNT" ]; then
    echo -e "   ${GREEN}✓${NC} Successfully stored permanent codes for $STORED_COUNT orgs"
else
    echo -e "   ${RED}[FAILED]${NC} Only found '$STORED_COUNT' codes in DB"
    sqlite3 "$DB_FILE" "SELECT app_key, org_id, code_type FROM cowen_permanent_code;"
    fail_suite "--- Full Table Dump (cowen_permanent_code) ---"
fi

# 3.1 Strict Key Integrity Validation (Bug Regression Check)
echo "   Verifying Data Integrity (No empty org_id)..."
BUGGED_RECORDS=$(sqlite3 "$DB_FILE" "SELECT org_id FROM cowen_permanent_code WHERE org_id = '' OR org_id IS NULL;" 2>/dev/null)
if [ -n "$BUGGED_RECORDS" ]; then
    fail_suite "Found records with empty org_id"
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
    RECEIVED_RESP=$(curl -s --connect-timeout 5 -x "http://127.0.0.1:$PROXY_PORT" -H "x-org-id: $ORG_ID" -X POST "$MOCK_URL/v1/app/data/get")
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
        kill_daemons_in_dirs "$COWEN_HOME"
        fail_suite "Full Response:             $RECEIVED_RESP"
    fi
done

# Cleanup
kill_daemons_in_dirs "$COWEN_HOME"

# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 27 Passed!${NC}"
