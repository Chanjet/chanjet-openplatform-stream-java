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

# 6. Install and enable plugins
PLUGIN_DIR="$HOME/.cowen/plugins"
if [ -d "lib" ] && [ "$(ls -A lib 2>/dev/null)" ]; then
    echo "🧩 Installing plugins..."
    mkdir -p "$PLUGIN_DIR"
    cp -r lib/* "$PLUGIN_DIR/"
    
    # Wait briefly for daemon to stabilize if just installed/started
    sleep 1
    
    for file in "$PLUGIN_DIR"/*; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            if [[ "$filename" != *.bundle ]]; then
                chmod +x "$file"
                echo "⚙️ Enabling plugin $filename..."
                "$INSTALL_DIR/$BINARY" plugins enable "$filename" > /dev/null 2>&1 || true
            fi
        fi
    done
    echo "✅ Plugins installed and enabled."
fi

echo -e "\n🎉 Installation complete! Run 'cowen --help' to get started."
