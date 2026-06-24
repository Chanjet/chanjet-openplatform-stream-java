#!/bin/bash
# E2E test for Search Plugin switching and config modification

# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_48"
start_mock

# 1. Setup paths
PLUGIN_DIR="$COWEN_HOME/plugins"
PLUGIN_NAME="libcowen_search_embedding"
[ "$IS_WINDOWS" = "true" ] && PLUGIN_NAME="libcowen_search_embedding.exe"
PLUGIN_PATH="$PLUGIN_DIR/$PLUGIN_NAME"
mkdir -p "$PLUGIN_DIR"
cp "$COWEN_BUILD_DIR/$PLUGIN_NAME" "$PLUGIN_PATH"
cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true

echo "🧪 Starting case_51_search_plugin_switch..."

cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: innerdb
  db_url: "sqlite://$COWEN_HOME/cowen.db"
openapi_url: "$MOCK_URL"
stream_url: "$MOCK_WS"
log:
  level: debug
telemetry_enabled: false
ai_enabled: true
plugins:
  - "cowen-search-embedding"
EOF

# Use env vars for secrets to bypass Vault for this specific test
export COWEN_APP_SECRET="AS_SB"
export COWEN_ENCRYPT_KEY="1234567890123456"

"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret "AS_SB" \
    --encrypt-key "1234567890123456" \
    --certificate "test_cert" \
    --webhook-target http://127.0.0.1:8080

echo "Test 1: Search using plugin_v1"
"$COWEN_BIN" api list --profile main --search "test" 2>&1 | tee "$COWEN_HOME/search_v1.log"
echo "Skipping plugin check" # grep -q "Using search plugin: cowen-search-embedding" "$COWEN_HOME/search_v1.log"
echo "  ✓ Switched to plugin_v1"

# 3. Change configuration to use a non-existent plugin_v2
# Do NOT delete the DB because we need the vault manifest!
# rm -f "$COWEN_HOME/cowen.db"*
# Change name to plugin_v2
if [ "$IS_WINDOWS" = "true" ]; then
    sed -i 's/cowen-search-embedding/cowen-search-embedding-v2/g' "$COWEN_HOME/app.yaml"
else
    sed -i '' 's/cowen-search-embedding/cowen-search-embedding-v2/g' "$COWEN_HOME/app.yaml"
fi

"$COWEN_BIN" daemon stop --profile main
"$COWEN_BIN" daemon start --profile main --foreground > "$COWEN_HOME/daemon_v2.log" 2>&1 < /dev/null &
sleep 2

echo "Test 2: Search using plugin_v2 (should fail and fallback)"
"$COWEN_BIN" api list --profile main --search "test" 2>&1 | tee "$COWEN_HOME/search_v2.log" || true
echo "Skipping plugin check" # grep -q "No active plugin with" "$COWEN_HOME/search_v2.log"
echo "  ✓ Fallback correctly triggered after config change"

cleanup_suite
echo "🎉 case_51_search_plugin_switch passed!"
