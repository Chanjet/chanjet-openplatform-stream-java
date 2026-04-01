#!/bin/bash
set -e

BINARY="owenc"
INSTALL_DIR="$HOME/.owenc/bin"

echo "🚀 Installing $BINARY to $INSTALL_DIR..."

# 1. Create directory
mkdir -p "$INSTALL_DIR"

# 2. Copy binary
cp "$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"
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
        echo -e "\n# owenc CLI\nexport PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
        echo "✅ PATH updated. Please run 'source $SHELL_RC' or restart terminal."
    fi
fi

# 4. Setup Shell Completion
echo "⚙️ Setting up shell completion..."
"$INSTALL_DIR/$BINARY" completion --install > /dev/null 2>&1 || true

echo -e "\n🎉 Installation complete! Run 'owenc --help' to get started."
