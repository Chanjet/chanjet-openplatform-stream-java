#!/bin/bash
set -e

# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_65"

echo "🧪 Starting case_65_plugin_usability..."

# Create the plugins directory and copy the signed plugin
PLUGIN_DIR="$COWEN_HOME/plugins"
mkdir -p "$PLUGIN_DIR"
if [ -f "$COWEN_BUILD_DIR/libcowen_search_embedding" ]; then
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
elif [ -f "$COWEN_BUILD_DIR/libcowen_search_embedding.exe" ]; then
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.exe" "$PLUGIN_DIR/"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true
fi
cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$PLUGIN_DIR/" 2>/dev/null || true

# Unset COWEN_DEV_MODE to force PKI signature verification
unset COWEN_DEV_MODE

# Enable the plugin
"$COWEN_BIN" plugins enable libcowen_search_embedding >/dev/null 2>&1 || true

# Run plugins list command and verify the plugin is active and enabled
OUTPUT=$("$COWEN_BIN" plugins list 2>&1)
echo "$OUTPUT"

if echo "$OUTPUT" | grep -q "libcowen_search_embedding" && echo "$OUTPUT" | grep -q "Yes" && ! echo "$OUTPUT" | grep -q "Failed"; then
    echo "✅ Plugin is loaded and enabled successfully."
else
    echo "❌ Plugin failed to load or is not enabled. Output was:"
    echo "$OUTPUT"
    exit 1
fi

cleanup_suite
echo "🎉 case_65_plugin_usability passed!"
