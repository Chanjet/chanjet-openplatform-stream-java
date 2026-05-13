#!/bin/bash
# Case 33: PostgreSQL Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 1 and Node 2 can share tokens via PostgreSQL.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via PostgreSQL.

source tests/e2e/scripts/common.sh

# Configuration
PG_PORT=5432
DB_NAME=$(get_case_db_name "case_33")

# Support both local brew (current user) and podman (postgres user)
if psql -d postgres -c "select 1" &> /dev/null; then
    PG_BASE_URL="postgres://$(whoami)@127.0.0.1:$PG_PORT"
    PG_CMD="psql -d postgres"
else
    PG_BASE_URL="postgres://postgres:password@127.0.0.1:$PG_PORT"
    export PGPASSWORD=password
    PG_CMD="psql -h 127.0.0.1 -p $PG_PORT -U postgres -d postgres"
fi

PG_URL="$PG_BASE_URL/$DB_NAME?sslmode=disable"

echo -e "${BOLD}1. Setup PostgreSQL Isolation and Node 1${NC}"
setup_workspace "case_33"

# Ensure PostgreSQL is up and create isolated DB
echo -n "  Preparing isolated PostgreSQL database '$DB_NAME'..."
if ! command -v psql &> /dev/null; then
    echo -e " ${YELLOW}[WARNING] psql not found, falling back to podman exec${NC}"
    podman exec cowen-postgres psql -U postgres -c "DROP DATABASE IF EXISTS $DB_NAME;" 2>/dev/null || true
    podman exec cowen-postgres psql -U postgres -c "CREATE DATABASE $DB_NAME;" 2>/dev/null || true
else
    $PG_CMD -c "DROP DATABASE IF EXISTS $DB_NAME;" 2>/dev/null || true
    $PG_CMD -c "CREATE DATABASE $DB_NAME;"
    echo -e " ${GREEN}[OK]${NC}"
fi

# Define nodes
export TEST_BASE="${TEST_BASE:-$(pwd)/target/cowen_tests}"
HOME_1="$TEST_BASE/.cowen_test_pg_node_1"
HOME_2="$TEST_BASE/.cowen_test_pg_node_2"

rm -rf "$HOME_1" "$HOME_2"
mkdir -p "$HOME_1" "$HOME_2"

start_mock

# --- Node 1: Initializer ---
export COWEN_HOME="$HOME_1"
cat > "$HOME_1/app.yaml" <<EOF
storage:
  store: postgres
  db_url: "$PG_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Node 1 initialization will now happen in a completely fresh database
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_PG \
    --app-secret AS_PG \
    --encrypt-key 1234567890123456 \
    --certificate CERT_PG \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --proxy-port 9293 > /dev/null

assert_pass "Node 1 initialized and linked to PostgreSQL"

# --- Node 2: Follower ---
echo -e "${BOLD}2. Setup Node 2 (No Init)${NC}"
export COWEN_HOME="$HOME_2"
cat > "$HOME_2/app.yaml" <<EOF
storage:
  store: postgres
  db_url: "$PG_URL"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF

# Node 2 should see 'main' profile immediately
PROFILES=$("$COWEN_BIN" profile list)
if [[ "$PROFILES" == *"main"* ]]; then
    echo -e "   ✓ Node 2 successfully discovered 'main' profile from PostgreSQL"
else
    echo -e "   ${RED}[FAILED]${NC} Node 2 could not see 'main' profile"
    exit 1
fi

# 3. Verify Token Synchronization
echo -e "${BOLD}3. Verify Token Synchronization${NC}"

# 1. Get initial token from Node 1
export COWEN_HOME="$HOME_1"
TOKEN_1=$(extract_token "main")
echo -e "   Node 1 Initial Token: ${BLUE}${TOKEN_1:0:15}...${NC}"

# 2. Get token from Node 2 (should read from DB)
export COWEN_HOME="$HOME_2"
TOKEN_2=$(extract_token "main")
echo -e "   Node 2 Initial Token: ${BLUE}${TOKEN_2:0:15}...${NC}"

if [ "$TOKEN_1" == "$TOKEN_2" ]; then
    echo -e "   ✓ Initial token synchronized via PostgreSQL"
else
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    exit 1
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$("$COWEN_BIN" auth token --profile main --refresh --format json | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token'))")
echo -e "   Node 1 New Token:     ${BLUE}${TOKEN_V2:0:15}...${NC}"

echo -e "${BOLD}5. Verify Node 2 Sync${NC}"
export COWEN_HOME="$HOME_2"
TOKEN_2_V2=$(extract_token "main")
echo -e "   Node 2 New Token:     ${BLUE}${TOKEN_2_V2:0:15}...${NC}"

if [ "$TOKEN_V2" == "$TOKEN_2_V2" ]; then
    echo -e "   ✓ Node 2 picked up refreshed token from Node 1 via PostgreSQL"
else
    echo -e "   ${RED}[FAILED]${NC} Node 2 token not synchronized after refresh"
    exit 1
fi

echo -e "\n${GREEN}🎊 Case 33 Passed!${NC}"
