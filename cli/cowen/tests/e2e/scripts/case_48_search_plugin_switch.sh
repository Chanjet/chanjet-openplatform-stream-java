#!/bin/bash
# E2E test for Search Plugin switching and config modification

# Source common utilities
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "51"
start_mock

# 1. Setup paths
PLUGIN_DIR="$COWEN_HOME/dist_assets"
PLUGIN_NAME="libcowen_search_embedding.dylib"
[ "$IS_WINDOWS" = "true" ] && PLUGIN_NAME="libcowen_search_embedding.dll"
PLUGIN_PATH="$PLUGIN_DIR/$PLUGIN_NAME"
mkdir -p "$PLUGIN_DIR"
cp "$COWEN_BUILD_DIR/$PLUGIN_NAME" "$PLUGIN_PATH"

echo "🧪 Starting case_51_search_plugin_switch..."

# 2. Setup config files directly (avoiding Vault manifest sync issues)
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: innerdb
  db_url: "sqlite://$COWEN_HOME/cowen.db"
log:
  level: debug
telemetry_enabled: false
ai_enabled: true
EOF

cat > "$COWEN_HOME/main.yaml" <<EOF
app_key: AK_SB
openapi_url: "$MOCK_URL"
stream_url: "$MOCK_URL"
webhook_target: http://127.0.0.1:8080
log:
  level: info
telemetry_enabled: false
ai_enabled: true
search:
  plugins:
    - name: "plugin_v1"
      path: "$PLUGIN_PATH"
      type: "embedding"
  enabled: ["plugin_v1"]
app_mode: self-built
EOF

# Use env vars for secrets to bypass Vault for this specific test
export COWEN_APP_SECRET="AS_SB"
export COWEN_ENCRYPT_KEY="1234567890123456"

echo "Test 1: Search using plugin_v1"
"$COWEN_BIN" api list --profile main --search "test" 2>&1 | tee "$COWEN_HOME/search_v1.log"
grep -q "Using search plugin: plugin_v1" "$COWEN_HOME/search_v1.log"
echo "  ✓ Switched to plugin_v1"

# 3. Change configuration to use a non-existent plugin_v2
# We also delete the DB to ensure manifest is reloaded from modified YAML
rm -f "$COWEN_HOME/cowen.db"* 
# Change name to plugin_v2 and path to invalid
if [ "$IS_WINDOWS" = "true" ]; then
    sed -i 's/plugin_v1/plugin_v2/g' "$COWEN_HOME/main.yaml"
    sed -i 's/path: .*/path: "C:\\non\\existent\\path"/g' "$COWEN_HOME/main.yaml"
else
    sed -i '' 's/plugin_v1/plugin_v2/g' "$COWEN_HOME/main.yaml"
    sed -i '' 's/path: .*/path: "\/non\/existent\/path"/g' "$COWEN_HOME/main.yaml"
fi

echo "Test 2: Search using plugin_v2 (should fail and fallback)"
"$COWEN_BIN" api list --profile main --search "test" 2>&1 | tee "$COWEN_HOME/search_v2.log"
grep -q "Failed to load search plugin 'plugin_v2'" "$COWEN_HOME/search_v2.log"
echo "  ✓ Fallback correctly triggered after config change"

cleanup_suite
echo "🎉 case_51_search_plugin_switch passed!"
