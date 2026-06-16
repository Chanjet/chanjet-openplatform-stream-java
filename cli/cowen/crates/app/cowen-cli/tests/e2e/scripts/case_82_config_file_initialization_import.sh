#!/bin/bash
set -e
# Case 82: Configuration file initialization, import, and conflict validation

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_82"
trap cleanup_suite EXIT
start_mock

PORT_P1=$(get_unused_port)
PROXY_PORT_P1=$(get_unused_port)
PORT_P2=$(get_unused_port)
PROXY_PORT_P2=$(get_unused_port)

echo "PORT_P1: $PORT_P1, PROXY_PORT_P1: $PROXY_PORT_P1"
echo "PORT_P2: $PORT_P2, PROXY_PORT_P2: $PROXY_PORT_P2"

# 1. Test Scenario 1: Init with a config file
echo "1. Testing initialization using a config file..."

TEMPLATE_P1="$COWEN_HOME/../p1_template.yaml"
cat <<EOF > "$TEMPLATE_P1"
app_key: "mock_app_key_1"
app_mode: "store-app"
webhook_target: "http://127.0.0.1:9299/callback"
proxy_port: $PROXY_PORT_P1
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:$PORT_P1"
  routes:
    - path: "/**"
      upstream: "http://127.0.0.1:9299"
storage:
  type: "sqlite"
log:
  level: "info"
EOF

"$COWEN_BIN" init --profile p1 \
    --file "$TEMPLATE_P1" \
    --app-secret "my_secret_key" \
    --encrypt-key "1234567890123456" >/dev/null

assert_pass "Initialized profile 'p1' using file template"

# Verify that configuration was parsed correctly
CFG=$("$COWEN_BIN" config --profile p1)
assert_match "$CFG" "mock_app_key_1" "Config has app_key mock_app_key_1"
assert_match "$CFG" "$PROXY_PORT_P1" "Config has proxy_port $PROXY_PORT_P1"
assert_match "$CFG" "bind_address: 127.0.0.1:$PORT_P1" "Config has bind_address"

# 1.5. Test config template export (PRD requirement)
echo "1.5. Testing config template export..."

# A. No profile (Default template)
TEMP_DEFAULT="$COWEN_HOME/../temp_default.yaml"
"$COWEN_BIN" config template > "$TEMP_DEFAULT"
assert_match "$(cat $TEMP_DEFAULT)" "app_key: \"mock_app_key\"" "Default template has default app_key"
assert_match "$(cat $TEMP_DEFAULT)" "app_mode: \"store-app\"" "Default template has default app_mode"
if grep -q "^proxy_port:" "$TEMP_DEFAULT"; then
    fail_suite "Default template should comment out optional proxy_port"
fi

# B. With configured profile 'p1' (Pre-filled non-sensitive values)
TEMP_P1_EXP="$COWEN_HOME/../temp_p1_exp.yaml"
"$COWEN_BIN" config template --profile p1 > "$TEMP_P1_EXP"
assert_match "$(cat $TEMP_P1_EXP)" "app_key: \"mock_app_key_1\"" "Pre-filled template has p1 app_key"
assert_match "$(cat $TEMP_P1_EXP)" "proxy_port: $PROXY_PORT_P1" "Pre-filled template pre-filled proxy_port"
assert_match "$(cat $TEMP_P1_EXP)" "bind_address: \"127.0.0.1:$PORT_P1\"" "Pre-filled template pre-filled bind_address"
if grep -q "^app_secret:" "$TEMP_P1_EXP" || grep -q "^encrypt_key:" "$TEMP_P1_EXP"; then
    fail_suite "Pre-filled template must not uncomment sensitive keys"
fi

# 2. Test Scenario 2: Re-initialization overrides config but keeps secrets
echo "2. Testing re-initialization override..."

# Modify p1 template: change proxy_port and webhook_target, but omit app_secret & encrypt_key
TEMPLATE_P1_NEW="$COWEN_HOME/../p1_template_new.yaml"
NEW_PROXY_PORT=$(get_unused_port)
cat <<EOF > "$TEMPLATE_P1_NEW"
app_key: "mock_app_key_1"
app_mode: "store-app"
webhook_target: "http://127.0.0.1:9299/callback_new"
proxy_port: $NEW_PROXY_PORT
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:$PORT_P1"
  routes:
    - path: "/**"
      upstream: "http://127.0.0.1:9299"
EOF

# Run init again on 'p1' with the new file, without secret options
"$COWEN_BIN" init --profile p1 \
    --file "$TEMPLATE_P1_NEW" >/dev/null

assert_pass "Re-initialized profile 'p1' using new file template"

CFG_NEW=$("$COWEN_BIN" config --profile p1)
assert_match "$CFG_NEW" "callback_new" "Config webhook_target was updated"
assert_match "$CFG_NEW" "$NEW_PROXY_PORT" "Config proxy_port was updated to $NEW_PROXY_PORT"

# Check if the secret is still active
assert_match "$CFG_NEW" "version:" "Config has version field"

# Verify that sensitive credentials were NOT overridden to empty value during re-init
SECRET_VAL_INIT=$("$COWEN_BIN" config get --profile p1 app_secret | tr -d '"')
if [ -z "$SECRET_VAL_INIT" ] || [ "$SECRET_VAL_INIT" = "null" ]; then
    fail_suite "app_secret was cleared or overridden to empty during re-init"
fi

# 3. Test Scenario 3: Config Import & deep merge
echo "3. Testing config import & merge..."

PATCH_P1="$COWEN_HOME/../p1_patch.yaml"
FINAL_PROXY_PORT=$(get_unused_port)
cat <<EOF > "$PATCH_P1"
proxy_port: $FINAL_PROXY_PORT
gateway:
  auth_routing:
    mode: "PERMISSIVE"
log:
  level: "debug"
EOF

# Start the daemon in the background to test hot-reload/merge broadcasting
"$COWEN_BIN" daemon start --profile p1
sleep 1

"$COWEN_BIN" config import --profile p1 --file "$PATCH_P1"

# Verify final proxy port and auth routing mode
CFG_PATCHED=$("$COWEN_BIN" config --profile p1)
assert_match "$CFG_PATCHED" "$FINAL_PROXY_PORT" "Config proxy_port was patched to $FINAL_PROXY_PORT"
assert_match "$CFG_PATCHED" "PERMISSIVE" "Config gateway.auth_routing.mode was patched to PERMISSIVE"

# Verify global log level in app.yaml
assert_match "$CFG_PATCHED" "level: debug" "Global log level was updated to debug"

# Verify that sensitive credentials were NOT overridden to empty value during import
SECRET_VAL_IMPORT=$("$COWEN_BIN" config get --profile p1 app_secret | tr -d '"')
if [ -z "$SECRET_VAL_IMPORT" ] || [ "$SECRET_VAL_IMPORT" = "null" ]; then
    fail_suite "app_secret was cleared or overridden to empty during import"
fi

# Stop the daemon
"$COWEN_BIN" daemon stop --profile p1 || true

# 4. Test Scenario 4: Distributed Conflict Interception
echo "4. Testing port conflict validation..."

# Profile 2 template with conflicting proxy_port
TEMPLATE_P2_CONFLICT="$COWEN_HOME/../p2_conflict.yaml"
cat <<EOF > "$TEMPLATE_P2_CONFLICT"
app_key: "mock_app_key_2"
app_mode: "store-app"
proxy_port: $FINAL_PROXY_PORT
EOF

if "$COWEN_BIN" init --profile p2 \
    --file "$TEMPLATE_P2_CONFLICT" \
    --app-secret "my_secret_key_2" \
    --encrypt-key "1234567890123456" 2>&1 | grep -q "Port conflict"; then
    echo -e "  ${GREEN}✓${NC} Blocked initialization due to proxy_port conflict"
else
    fail_suite "Should have blocked initialization due to proxy_port conflict"
fi

# Profile 2 template with conflicting gateway port
TEMPLATE_P2_CONFLICT_GW="$COWEN_HOME/../p2_conflict_gw.yaml"
cat <<EOF > "$TEMPLATE_P2_CONFLICT_GW"
app_key: "mock_app_key_2"
app_mode: "store-app"
gateway:
  bind_address: "127.0.0.1:$PORT_P1"
EOF

if "$COWEN_BIN" init --profile p2 \
    --file "$TEMPLATE_P2_CONFLICT_GW" \
    --app-secret "my_secret_key_2" \
    --encrypt-key "1234567890123456" 2>&1 | grep -q "Port conflict"; then
    echo -e "  ${GREEN}✓${NC} Blocked initialization due to gateway port conflict"
else
    fail_suite "Should have blocked initialization due to gateway port conflict"
fi

# Confirm p2 was not initialized
LIST_P2=$("$COWEN_BIN" profile list)
if echo "$LIST_P2" | grep -q "p2"; then
    fail_suite "profile 'p2' should not have been initialized due to port conflict"
fi

# 5. Test Scenario 5: Environment Variable Override has highest priority
echo "5. Testing environment variable overrides..."

# Terminate any running master daemon first to ensure a fresh start with environment variables
if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
    PID=$(cat "$COWEN_HOME/master_daemon.pid" | head -n 1 2>/dev/null)
    if [ -n "$PID" ]; then
        kill -9 "$PID" >/dev/null 2>&1 || true
    fi
fi
pkill -9 -f "cowen_daemon_case_82" || true
pkill -9 -f "cowen_case_82" || true
sleep 1.5

PORT_ENV=$(get_unused_port)
echo "Starting daemon with COWEN_GATEWAY_BIND=127.0.0.1:$PORT_ENV"

# Start the daemon with environment variable overrides
COWEN_GATEWAY_BIND="127.0.0.1:$PORT_ENV" "$COWEN_BIN" daemon start --profile p1
sleep 1.5

# Test 1: Verify that CLI (requesting active configuration) sees the overridden value
CFG_ENV=$("$COWEN_BIN" config --profile p1)
assert_match "$CFG_ENV" "bind_address: 127.0.0.1:$PORT_ENV" "Running daemon config has overridden bind_address"

# Test 2: Verify that the Gateway is indeed listening on the overridden port
# /v1/mock/ping is bypassed route from templates
RES_ENV=$(curl -s "http://127.0.0.1:$PORT_ENV/v1/mock/ping")
assert_match "$RES_ENV" "status" "Gateway responded on overridden port $PORT_ENV"

# Stop the daemon
"$COWEN_BIN" daemon stop --profile p1 || true

echo -e "\n${GREEN}🎊 Case 82 Passed!${NC}"
exit 0
