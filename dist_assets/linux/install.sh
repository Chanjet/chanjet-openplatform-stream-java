#!/bin/bash
set -e

BINARY="owenc"
INSTALL_DIR="/usr/local/bin"

echo "🚀 Installing $BINARY to $INSTALL_DIR..."

# 1. Check permissions
if [ ! -w "$INSTALL_DIR" ]; then
    echo "❌ Error: No write permission to $INSTALL_DIR. Please run with sudo."
    exit 1
fi

# 2. Copy binary
cp "$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"
echo "✅ Binary installed successfully."

# 3. Setup Shell Completion (Optional)
if [ -n "$SHELL" ]; then
    echo "⚙️ Setting up shell completion..."
    "$INSTALL_DIR/$BINARY" completion --install > /dev/null 2>&1 || true
fi

echo -e "\n🎉 Installation complete! Run 'owenc --help' to get started."
