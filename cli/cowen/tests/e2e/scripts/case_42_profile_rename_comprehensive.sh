#!/bin/bash
# Case 42: Comprehensive Profile Rename Test across all storage modes

source tests/e2e/scripts/common.sh

test_rename_for_storage() {
    local mode=$1
    local db_url=$2
    local extra_init_args=$3
    
    echo -e "${BOLD}▶ Testing rename in mode: $mode${NC}"
    
    # 1. Setup Workspace
    setup_workspace "rename_$mode"
    start_mock
    
    # 2. Configure app.yaml
    if [ "$mode" == "local" ]; then
        cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: local
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF
    else
        cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: $mode
  db_url: "$db_url"
log:
  level: debug
telemetry_enabled: false
ai_enabled: false
EOF
    fi

    # 3. Initialize profile 'old_prof'
    # Use standard arguments that we can verify later
    "$COWEN_BIN" init --profile old_prof \
        --app-mode self-built \
        --app-key "AK_RENAME_$mode" \
        --app-secret "AS_RENAME_$mode" \
        --encrypt-key 1234567890123456 \
        --certificate "CERT_RENAME_$mode" \
        --openapi-url $MOCK_URL \
        --stream-url $MOCK_WS $extra_init_args >/dev/null
    assert_pass "Initialized profile 'old_prof' for $mode"

    # 4. Stop daemon to ensure no background interference
    "$COWEN_BIN" daemon stop --profile old_prof >/dev/null 2>&1 || true

    # 5. Rename to 'new_prof'
    "$COWEN_BIN" profile rename old_prof new_prof >/dev/null
    assert_pass "Renamed 'old_prof' to 'new_prof' in $mode"

    # 6. Verify 'old_prof' is gone
    LIST=$("$COWEN_BIN" profile list)
    if echo "$LIST" | grep -q "old_prof"; then
        echo -e "  ${RED}✗${NC} 'old_prof' still exists in $mode"
        exit 1
    fi
    echo -e "  ${GREEN}✓${NC} 'old_prof' removed from list"

    # 7. Verify 'new_prof' exists and has data
    assert_match "$LIST" "new_prof" "List contains 'new_prof'"
    
    CFG=$("$COWEN_BIN" config --profile new_prof)
    assert_match "$CFG" "AK_RENAME_$mode" "Data (AppKey) migrated to 'new_prof'"
    
    # 8. Extra check for Local mode: file exists
    if [ "$mode" == "local" ]; then
        if [ ! -f "$COWEN_HOME/new_prof.yaml" ]; then
            echo -e "  ${RED}✗${NC} new_prof.yaml not found in local mode"
            exit 1
        fi
        if [ -f "$COWEN_HOME/old_prof.yaml" ]; then
            echo -e "  ${RED}✗${NC} old_prof.yaml still exists in local mode"
            exit 1
        fi
        echo -e "  ${GREEN}✓${NC} Local file renamed correctly"
    fi

    cleanup_suite
}

# --- 1. Local Mode (YAML) ---
test_rename_for_storage "local" ""

# --- 2. SQLite Mode (InnerDB) ---
DB_FILE="target/cowen_tests/case_42_sqlite.db"
rm -f "$DB_FILE"
test_rename_for_storage "innerdb" "sqlite://$DB_FILE"

# --- 3. Redis Mode ---
if command -v redis-server &> /dev/null; then
    REDIS_PORT=6389
    REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
    lsof -ti ":$REDIS_PORT" | xargs kill -9 2>/dev/null || true
    redis-server --port $REDIS_PORT --save "" --daemonize yes
    sleep 2
    clear_redis "$REDIS_URL"
    
    test_rename_for_storage "redis" "$REDIS_URL"
    
    redis-cli -p $REDIS_PORT shutdown || true
else
    echo -e "${YELLOW}  [SKIP] Redis not found, skipping Redis rename test${NC}"
fi

# --- 4. MySQL Mode ---
MYSQL_PORT=3306
DB_NAME="cowen_test_rename_42"
if mysql -u root -h 127.0.0.1 -P $MYSQL_PORT -e "select 1" < /dev/null &> /dev/null; then
    MYSQL_URL="mysql://root@127.0.0.1:$MYSQL_PORT/$DB_NAME"
    mysql -u root -h 127.0.0.1 -P $MYSQL_PORT -e "DROP DATABASE IF EXISTS $DB_NAME; CREATE DATABASE $DB_NAME;" < /dev/null
    test_rename_for_storage "mysql" "$MYSQL_URL"
elif mysql -u root -proot -h 127.0.0.1 -P $MYSQL_PORT -e "select 1" < /dev/null &> /dev/null; then
    MYSQL_URL="mysql://root:root@127.0.0.1:$MYSQL_PORT/$DB_NAME"
    mysql -u root -proot -h 127.0.0.1 -P $MYSQL_PORT -e "DROP DATABASE IF EXISTS $DB_NAME; CREATE DATABASE $DB_NAME;" < /dev/null
    test_rename_for_storage "mysql" "$MYSQL_URL"
else
    echo -e "${YELLOW}  [SKIP] Local MySQL not found or inaccessible, skipping MySQL rename test${NC}"
fi

# --- 5. Postgres Mode ---
PG_PORT=5432
# Support both local brew (current user) and postgres user
CURRENT_USER=$(whoami)
if psql -h 127.0.0.1 -p $PG_PORT -d postgres -w -c "select 1" &> /dev/null; then
    PG_CMD="psql -h 127.0.0.1 -p $PG_PORT -d postgres -w"
    PG_CONN_URL="postgres://127.0.0.1:$PG_PORT/$DB_NAME"
elif psql -U $CURRENT_USER -h 127.0.0.1 -p $PG_PORT -d postgres -w -c "select 1" &> /dev/null; then
    PG_CMD="psql -U $CURRENT_USER -h 127.0.0.1 -p $PG_PORT -d postgres -w"
    PG_CONN_URL="postgres://$CURRENT_USER@127.0.0.1:$PG_PORT/$DB_NAME"
elif PGPASSWORD=password psql -U postgres -h 127.0.0.1 -p $PG_PORT -d postgres -w -c "select 1" &> /dev/null; then
    export PGPASSWORD=password
    PG_CMD="psql -U postgres -h 127.0.0.1 -p $PG_PORT -d postgres -w"
    PG_CONN_URL="postgres://postgres:password@127.0.0.1:$PG_PORT/$DB_NAME"
fi

if [ -n "$PG_CMD" ]; then
    safe_psql_exec "DROP DATABASE IF EXISTS $DB_NAME;" "postgres" >/dev/null 2>&1 || true
    if safe_psql_exec "CREATE DATABASE $DB_NAME;" "postgres"; then
        sleep 2
        test_rename_for_storage "postgres" "$PG_CONN_URL"
    fi
else
    echo -e "${YELLOW}  [SKIP] Postgres not found, skipping Postgres rename test${NC}"
fi

echo -e "\n${GREEN}${BOLD}🎊 Case 42 Comprehensive Rename Passed!${NC}"
exit 0
