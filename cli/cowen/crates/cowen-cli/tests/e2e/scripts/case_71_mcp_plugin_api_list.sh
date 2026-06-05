#!/bin/bash
set -e
# E2E test for MCP plugin api_list

# Source common utilities
if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_71"
start_mock

PLUGIN_DIR="$COWEN_HOME/plugins"
mkdir -p "$PLUGIN_DIR"

BIN_NAME="cowen-mcp-plugin"
[ "$IS_WINDOWS" = "true" ] && BIN_NAME="cowen-mcp-plugin.exe"

if [ -f "$COWEN_BUILD_DIR/$BIN_NAME" ]; then
    cp "$COWEN_BUILD_DIR/$BIN_NAME" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/cowen-mcp-plugin.bundle" "$PLUGIN_DIR/" 2>/dev/null || cp "target/release/cowen-mcp-plugin.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
else
    cp "$COWEN_BUILD_DIR/deps/$BIN_NAME" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/deps/cowen-mcp-plugin.bundle" "$PLUGIN_DIR/" 2>/dev/null || cp "target/release/cowen-mcp-plugin.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
fi

echo "🧪 Starting case_71_mcp_plugin_api_list..."

# 1. Use 'init' to properly set up the profile in the Vault
# This will initialize the 'main' profile
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "test-cert" --openapi-url "$MOCK_URL" --stream-url "$MOCK_URL" --encrypt-key "1234567890123456" \
    --webhook-target "http://127.0.0.1:8080" --no-telemetry >/dev/null

# Make sure 'main' is set as current profile (if needed, but plugins run should inject it if we specify --profile main)
# Or we can just pass --profile main to plugins run
# 2. Test MCP plugin API list
echo "Test 1: Run cowen_api_list through MCP plugin"

# Send JSON-RPC request to stdin
OUTPUT=$(echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "cowen_api_list", "arguments": {"search": "账套"}}}' | "$COWEN_BIN" --profile main plugins run cowen-mcp-plugin -- server)

if echo "$OUTPUT" | grep -q "OAuth2 session missing or expired for profile 'default_tenant'"; then
    echo "❌ Error reproduced: MCP plugin ignored COWEN_PROFILE and used 'default_tenant'!"
    echo "Output: $OUTPUT"
    exit 1
fi

if ! echo "$OUTPUT" | grep -q "\"total\":"; then
    echo "❌ Expected API list output not found!"
    echo "Output: $OUTPUT"
    exit 1
fi

echo "✅ MCP plugin API list successful."

cleanup_suite
echo "🎉 case_71_mcp_plugin_api_list passed!"
