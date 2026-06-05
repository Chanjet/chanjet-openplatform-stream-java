#!/bin/bash
set -e
# E2E test for Plugins Management Commands (list, enable, disable)

# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_59"

echo "🧪 Starting case_59_plugins_management..."

# 1. Initialize profile
"$COWEN_BIN" init --profile main --app-mode self-built --app-key "AK_SB" --app-secret "AS_SB" \
    --certificate "test-cert" --openapi-url "http://127.0.0.1:10080" --stream-url "http://127.0.0.1:10080" --encrypt-key "1234567890123456" \
    --webhook-target "http://127.0.0.1:8080" --no-telemetry >/dev/null

PLUGIN_DIR="$COWEN_HOME/plugins"
PLUGIN_NAME="libcowen_search_embedding"
if [ "$IS_WINDOWS" = "true" ]; then
    PLUGIN_NAME="libcowen_search_embedding.exe"
fi
PLUGIN_PATH="$PLUGIN_DIR/$PLUGIN_NAME"

PLUGIN_ID="libcowen_search_embedding"

# 2. Test Empty List
echo "Test 1: Empty plugins list"
mkdir -p "$PLUGIN_DIR"
OUTPUT=$("$COWEN_BIN" plugins list --profile main)
if ! echo "$OUTPUT" | grep -q "(No executable plugins found)"; then
    echo "❌ Failed to report empty plugins list. Output:"
    echo "$OUTPUT"
    exit 1
fi
echo "✅ Empty list verified."

# 3. Copy plugin and test list
echo "Test 2: List plugins"
if [ -f "$COWEN_BUILD_DIR/$PLUGIN_NAME" ]; then
    cp "$COWEN_BUILD_DIR/$PLUGIN_NAME" "$PLUGIN_PATH"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
else
    cp "$COWEN_BUILD_DIR/deps/$PLUGIN_NAME" "$PLUGIN_PATH"
    cp "$COWEN_BUILD_DIR/deps/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
fi

OUTPUT=$("$COWEN_BIN" plugins list --profile main)
if ! echo "$OUTPUT" | grep "$PLUGIN_ID" | grep -q "SearchProvider"; then
    echo "❌ Failed to list plugin info. Output:"
    echo "$OUTPUT"
    exit 1
fi
echo "✅ Plugin list verified."

# 4. Test Enable Plugin
echo "Test 3: Enable plugin"
"$COWEN_BIN" plugins enable "$PLUGIN_ID" --profile main >/dev/null
OUTPUT=$("$COWEN_BIN" plugins list --profile main)
if ! echo "$OUTPUT" | grep "$PLUGIN_ID" | grep -q "Yes"; then
    echo "❌ Failed to enable plugin. Output:"
    echo "$OUTPUT"
    exit 1
fi
echo "✅ Plugin enabled successfully."

# 5. Test Disable Plugin
echo "Test 4: Disable plugin"
"$COWEN_BIN" plugins disable "$PLUGIN_ID" --profile main >/dev/null
OUTPUT=$("$COWEN_BIN" plugins list --profile main)
if ! echo "$OUTPUT" | grep "$PLUGIN_ID" | grep -q "No"; then
    echo "❌ Failed to disable plugin. Output:"
    echo "$OUTPUT"
    exit 1
fi
echo "✅ Plugin disabled successfully."

cleanup_suite
echo "🎉 case_59_plugins_management passed!"
