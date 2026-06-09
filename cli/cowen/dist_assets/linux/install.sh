#!/bin/bash
set -e

BINARY="cowen"
INSTALL_DIR="$HOME/.cowen/bin"

echo "🚀 Installing $BINARY to $INSTALL_DIR..."

# 1. Create directory
mkdir -p "$INSTALL_DIR"

# 2. Copy binary
cp "$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"
if [ -f "cowen-daemon" ]; then
    echo "🚀 Installing cowen-daemon to $INSTALL_DIR..."
    cp "cowen-daemon" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/cowen-daemon"
fi
echo "✅ Binary installed successfully."

# 3. Add to PATH if not already there
SHELL_RC=""
if [[ "$SHELL" == *"zsh"* ]]; then
    SHELL_RC="$HOME/.zshrc"
elif [[ "$SHELL" == *"bash"* ]]; then
    SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ] && [ -f "$SHELL_RC" ]; then
    if ! grep -q "$INSTALL_DIR" "$SHELL_RC"; then
        echo "Adding $INSTALL_DIR to PATH in $SHELL_RC..."
        echo -e "\n# cowen CLI\nexport PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
        echo "✅ PATH updated. Please run 'source $SHELL_RC' or restart terminal."
    fi
fi

echo "⚙️ Setting up shell completion..."
"$INSTALL_DIR/$BINARY" completion --install > /dev/null 2>&1 || true

# 5. Setup Autostart Service
echo "📟 Setting up autostart service..."
killall cowen-daemon > /dev/null 2>&1 || true
"$INSTALL_DIR/$BINARY" daemon service install > /dev/null 2>&1 || true

if [ -d "system_plugins" ]; then
    echo "📦 Installing system plugins..."
    mkdir -p "$HOME/.cowen/system_plugins"
    cp -r system_plugins/* "$HOME/.cowen/system_plugins/"
fi

# 6. Install and enable AI search plugin
PLUGIN_DIR="$HOME/.cowen/plugins"
if [ -d "lib" ] && ls lib/libcowen_search_embedding >/dev/null 2>&1; then
    echo "🧩 Installing AI search plugin..."
    mkdir -p "$PLUGIN_DIR"
    cp lib/libcowen_search_embedding "$PLUGIN_DIR/"
    if [ -f "lib/libcowen_search_embedding.bundle" ]; then
        cp lib/libcowen_search_embedding.bundle "$PLUGIN_DIR/"
    fi
    chmod +x "$PLUGIN_DIR/libcowen_search_embedding"
    
    echo "⚙️ Enabling AI search plugin..."
    # Wait briefly for daemon to stabilize if just installed/started
    sleep 1
    "$INSTALL_DIR/$BINARY" plugins enable libcowen_search_embedding > /dev/null 2>&1 || true
    echo "✅ AI search plugin installed and enabled."
fi

echo -e "\n🎉 Installation complete! Run 'cowen --help' to get started."
