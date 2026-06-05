#!/bin/bash
set -e
# E2E test for Composite Plugin loading and capabilities

# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_47"
start_mock

# 1. Setup paths
PLUGIN_DIR="$COWEN_HOME/dist_assets"
LIB_NAME="libcowen_search_embedding.bundle"
[ "$IS_WINDOWS" = "true" ] && LIB_NAME="libcowen_search_embedding.bundle"

echo "🧪 Starting case_50_search_plugins_composite..."

# 1. Use 'init' to properly set up the profile in the Vault
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "test-cert" --openapi-url "$MOCK_URL" --stream-url "$MOCK_URL" --encrypt-key "1234567890123456" \
    --webhook-target "http://127.0.0.1:8080" --no-telemetry >/dev/null

# 2. Setup multiple plugins
rm -rf "$PLUGIN_DIR"
mkdir -p "$PLUGIN_DIR"
if [ "$IS_WINDOWS" = "true" ]; then
    cp "$COWEN_BUILD_DIR/$LIB_NAME" "$PLUGIN_DIR/libembedding_a.exe"
    cp "$COWEN_BUILD_DIR/$LIB_NAME" "$PLUGIN_DIR/libranker_b.exe"
else
    cp "$COWEN_BUILD_DIR/$LIB_NAME" "$PLUGIN_DIR/libembedding_a"
    cp "$COWEN_BUILD_DIR/$LIB_NAME" "$PLUGIN_DIR/libranker_b"
fi

# 3. Verify search works with plugins present
echo "Test: Verify search with multiple plugins present"
"$COWEN_BIN" api list --profile main --search "Order"

cleanup_suite
echo "🎉 case_50_search_plugins_composite passed!"
