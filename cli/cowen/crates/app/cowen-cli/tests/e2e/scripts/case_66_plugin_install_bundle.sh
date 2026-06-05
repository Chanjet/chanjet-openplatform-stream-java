#!/bin/bash
set -e

# Source common utilities
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_66"

echo "🧪 Starting case_66_plugin_install_bundle..."

# Create a temporary source directory for the plugin installation test
TMP_PLUGIN_SRC="$COWEN_HOME/plugin_source"
mkdir -p "$TMP_PLUGIN_SRC"

# Copy the built plugin and bundle to the temporary source directory
if [ -f "$COWEN_BUILD_DIR/libcowen_search_embedding" ]; then
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding" "$TMP_PLUGIN_SRC/"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$TMP_PLUGIN_SRC/" || { echo "❌ Missing .bundle file in $COWEN_BUILD_DIR"; exit 1; }
    PLUGIN_EXT=""
elif [ -f "$COWEN_BUILD_DIR/libcowen_search_embedding.exe" ]; then
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.exe" "$TMP_PLUGIN_SRC/"
    cp "$COWEN_BUILD_DIR/libcowen_search_embedding.bundle" "$TMP_PLUGIN_SRC/" || { echo "❌ Missing .bundle file in $COWEN_BUILD_DIR"; exit 1; }
    PLUGIN_EXT="exe"
fi

PLUGIN_TARGET_DIR="$COWEN_HOME/plugins"

if [ -n "$PLUGIN_EXT" ]; then
    PLUGIN_SRC_PATH="$TMP_PLUGIN_SRC/libcowen_search_embedding.$PLUGIN_EXT"
    PLUGIN_TARGET_PATH="$PLUGIN_TARGET_DIR/libcowen_search_embedding.$PLUGIN_EXT"
else
    PLUGIN_SRC_PATH="$TMP_PLUGIN_SRC/libcowen_search_embedding"
    PLUGIN_TARGET_PATH="$PLUGIN_TARGET_DIR/libcowen_search_embedding"
fi

# Clean up existing plugins directory just to be safe
rm -rf "$PLUGIN_TARGET_DIR"

echo "Installing plugin from $PLUGIN_SRC_PATH..."
"$COWEN_BIN" plugins install "$PLUGIN_SRC_PATH"

# Verify that both the executable and the .bundle were copied to $COWEN_HOME/plugins
if [ ! -f "$PLUGIN_TARGET_PATH" ]; then
    echo "❌ Plugin binary was not installed to $PLUGIN_TARGET_DIR."
    exit 1
fi

if [ ! -f "$PLUGIN_TARGET_DIR/libcowen_search_embedding.bundle" ]; then
    echo "❌ Plugin bundle (.bundle) was NOT installed to $PLUGIN_TARGET_DIR!"
    exit 1
fi

echo "✅ Both plugin binary and bundle were successfully installed."

# Verify the plugin can be listed without signature failure (since bundle is present)
unset COWEN_DEV_MODE
OUTPUT=$("$COWEN_BIN" plugins list 2>&1)
echo "$OUTPUT"

if echo "$OUTPUT" | grep -q "libcowen_search_embedding" && ! echo "$OUTPUT" | grep -q "Failed"; then
    echo "✅ Plugin is loaded and verified successfully after install."
else
    echo "❌ Plugin failed to load after install. Output was:"
    echo "$OUTPUT"
    exit 1
fi

cleanup_suite
echo "🎉 case_66_plugin_install_bundle passed!"
