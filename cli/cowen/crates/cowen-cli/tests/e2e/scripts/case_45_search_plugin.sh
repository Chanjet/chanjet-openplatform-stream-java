#!/bin/bash
set -e
# E2E test for Search Plugin loading and fallback

# Source common utilities
# Support both direct run and parallel runner (which might copy the script)
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_45"
start_mock

# 1. Setup paths
PLUGIN_DIR="$COWEN_HOME/plugins"
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

# We must copy both the binary and the bundle to the plugins directory
BIN_NAME="libcowen_search_embedding"
[ "$IS_WINDOWS" = "true" ] && BIN_NAME="libcowen_search_embedding.exe"

if [ -f "$COWEN_BUILD_DIR/$BIN_NAME" ]; then
    cp "$COWEN_BUILD_DIR/$BIN_NAME" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
else
    cp "$COWEN_BUILD_DIR/deps/$BIN_NAME" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/deps/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
fi

# Enable the plugin
"$COWEN_BIN" plugins enable "libcowen_search_embedding" >/dev/null

# Run search
OUTPUT=$("$COWEN_BIN" api list --profile main --search "Order")

if ! echo "$OUTPUT" | grep -q "Using search plugin"; then
    echo "❌ Semantic search plugin was NOT used during search!"
    echo "Output: $OUTPUT"
    exit 1
fi

echo "✅ Plugin loaded successfully."

cleanup_suite
echo "🎉 case_45_search_plugin passed!"
