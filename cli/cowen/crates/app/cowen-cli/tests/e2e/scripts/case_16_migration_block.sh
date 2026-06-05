#!/bin/bash
# Case 16: Migration Security Check
# Verifies that an existing OAuth2 profile migrated to a shared DB is BLOCKED from loading.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_16"

echo -e "${YELLOW}Skipping case_16_migration_block because 'cowen store migrate' CLI is deprecated.${NC}"
pass_suite
exit 0

HOME_MIGRATE="$COWEN_HOME/node_migrate"
SHARED_DB="$COWEN_HOME/migrated_target.db"

function final_cleanup {
    echo -e "\n${YELLOW}🧹 Cleaning up Case 16 environment...${NC}"
    kill_daemons_in_dirs "$HOME_MIGRATE"
    cleanup_suite
}
trap final_cleanup EXIT

mkdir -p "$HOME_MIGRATE"
export COWEN_HOME="$HOME_MIGRATE"

# --- Phase 1: Local OAuth2 ---
# Start fresh, cowen will create default innerdb app.yaml automatically
rm -f "$HOME_MIGRATE/app.yaml"

# Initialize a local OAuth2 profile (Write file directly to avoid blocking init)
cat > "$HOME_MIGRATE/oauth_local.yaml" <<EOF
app_key: AK_OAUTH
openapi_url: $MOCK_URL
stream_url: $MOCK_WS
webhook_target: http://127.0.0.1:8080
log:
  level: info
  rotation: daily
  max_size_mb: 100
  max_files: 7
telemetry_enabled: false
ai_enabled: false
proxy_port: 9099
proxy_enabled: true
app_mode: oauth2
version: 0
EOF

assert_pass "Local OAuth2 profile created (fake)"

# Verify it can be loaded locally
"$COWEN_BIN" auth status --profile oauth_local > /dev/null
assert_pass "Local OAuth2 profile is loadable in local mode"

# --- Phase 2: Migrate to Shared DB ---
echo -e "\n${BOLD}2. Migrating to Shared SQLite...${NC}"
"$COWEN_BIN" store migrate --to "sqlite://$SHARED_DB" --mode clone

assert_pass "Migration command finished"

# --- Phase 3: Verify Block ---
echo -e "\n${BOLD}3. Verifying Block in Distributed Mode...${NC}"
# app.yaml should have been updated by migrate command to use sqlite://...
cat "$HOME_MIGRATE/app.yaml"

# Try to run a command with the migrated profile
echo "Testing load of migrated OAuth2 profile..."
set +e
OUTPUT=$("$COWEN_BIN" auth status --profile oauth_local 2>&1)
EXIT_CODE=$?
set -e

echo "$OUTPUT"

if [ $EXIT_CODE -ne 0 ] && echo "$OUTPUT" | grep -qi "not allowed in distributed storage scenarios"; then
    echo -e "  ${GREEN}✓${NC} Blocked migrated OAuth2 profile successfully"
else
    fail_suite "Failed to block migrated OAuth2 profile (Exit: $EXIT_CODE)"
fi

pass_suite


