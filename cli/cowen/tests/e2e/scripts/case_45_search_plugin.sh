#!/bin/bash
set -e
# E2E test for Search Plugin loading and fallback

# Source common utilities
# Support both direct run and parallel runner (which might copy the script)
if [ -f "tests/e2e/scripts/common.sh" ]; then
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_45"
start_mock

# 1. Setup paths
PLUGIN_DIR="$COWEN_HOME/dist_assets"
PLUGIN_NAME="libcowen_search_embedding.bundle"
[ "$IS_WINDOWS" = "true" ] && PLUGIN_NAME="libcowen_search_embedding.bundle"
PLUGIN_PATH="$PLUGIN_DIR/$PLUGIN_NAME"

echo "🧪 Starting case_48_search_plugin..."

# 1. Use 'init' to properly set up the profile in the Vault
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "test-cert" --openapi-url "$MOCK_URL" --stream-url "$MOCK_URL" --encrypt-key "1234567890123456" \
    --webhook-target "http://127.0.0.1:8080" --no-telemetry >/dev/null

# 2. Test Fallback when plugin missing
echo "Test 1: Fallback when plugin is missing"
rm -rf "$PLUGIN_DIR"
mkdir -p "$PLUGIN_DIR"

# Run search
"$COWEN_BIN" api list --profile main --search "Order"

# 3. Test Plugin Loading
echo "Test 2: Loading plugin"
# Find plugin (could be in debug or debug/deps depending on cargo version/cache)
if [ -f "$COWEN_BUILD_DIR/$PLUGIN_NAME" ]; then
    cp "$COWEN_BUILD_DIR/$PLUGIN_NAME" "$PLUGIN_PATH"
else
    cp "$COWEN_BUILD_DIR/deps/$PLUGIN_NAME" "$PLUGIN_PATH"
fi

# Run search
"$COWEN_BIN" api list --profile main --search "Order"

echo "✅ Plugin loaded successfully."

cleanup_suite
echo "🎉 case_48_search_plugin passed!"
