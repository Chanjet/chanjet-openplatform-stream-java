#!/bin/bash
set -e
# Case 32: PostgreSQL Shared Storage & Distributed Token Synchronization
# Verifies:
#   1. Node 1 and Node 2 can share tokens via PostgreSQL.
#   2. Token refresh on Node 1 is immediately visible to Node 2 via PostgreSQL.

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Configuration
PG_PORT=5432
DB_HOST=${DB_HOST:-127.0.0.1}
DB_NAME=$(get_case_db_name "case_32")

# Ensure PostgreSQL is ready
if ! wait_for_postgres "$DB_HOST" "$PG_PORT"; then
    exit 1
fi

# Detect Auth Credentials
if PGPASSWORD=password psql -h "$DB_HOST" -U postgres -d postgres -c "select 1" &> /dev/null; then
    PG_BASE_URL="postgres://postgres:password@$DB_HOST:$PG_PORT"
    export PGPASSWORD=password
elif psql -h "$DB_HOST" -d postgres -c "select 1" &> /dev/null; then
    PG_BASE_URL="postgres://$DB_HOST:$PG_PORT"
else
    # Fallback to default postgres/password if unsure
    PG_BASE_URL="postgres://postgres:password@$DB_HOST:$PG_PORT"
    export PGPASSWORD=password
fi

PG_URL="$PG_BASE_URL/$DB_NAME?sslmode=disable"

echo -e "${BOLD}1. Setup PostgreSQL Isolation and Node 1${NC}"
setup_workspace "case_32"

# Ensure PostgreSQL is up and create isolated DB
echo -n "  Preparing isolated PostgreSQL database '$DB_NAME'..."
if ! command -v psql &> /dev/null; then
    echo -e " ${RED}[ERROR] psql command not found.${NC}"
    exit 1
else
    safe_psql_exec "DROP DATABASE IF EXISTS $DB_NAME;" "postgres" >/dev/null 2>&1 || true
    if safe_psql_exec "CREATE DATABASE $DB_NAME;" "postgres"; then
        echo -e " ${GREEN}[OK]${NC}"
        sleep 2
    else
        echo -e " ${RED}[FAILED]${NC} Could not create database $DB_NAME"
        exit 1
    fi
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
# 🚀 Fix: Added retry loop for shared storage propagation in QEMU/Linux
echo -n "   Node 2 fetching token from shared PostgreSQL..."
TOKEN_2=""
for i in {1..10}; do
    export COWEN_HOME="$HOME_2"
    TOKEN_2=$(extract_token "main")
    if [ "$TOKEN_1" == "$TOKEN_2" ]; then
        echo -e " ${GREEN}[OK]${NC}"
        break
    fi
    echo -n "."
    sleep 1
done

if [ "$TOKEN_1" != "$TOKEN_2" ]; then
    echo -e "   ${RED}[FAILED]${NC} Tokens mismatched between nodes"
    echo "     Node 1: $TOKEN_1"
    echo "     Node 2: $TOKEN_2"
    exit 1
fi

echo -e "${BOLD}4. Refresh Token on Node 1${NC}"
export COWEN_HOME="$HOME_1"
TOKEN_V2=$(extract_token "main" --refresh)
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

echo -e "\n${GREEN}🎊 Case 32 Passed!${NC}"
