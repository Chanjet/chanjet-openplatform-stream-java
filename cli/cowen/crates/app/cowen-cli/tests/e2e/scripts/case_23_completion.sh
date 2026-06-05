#!/bin/bash
set -e
# Case 23: Shell Completion Support
# Verifies:
#   1. 'completion <shell>' generates a non-empty script.
#   2. 'completion --install' modifies the shell rc file.
#   3. 'completion --uninstall' removes the configuration.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_23"

# Mock the shell RC file
MOCK_ZSHRC="$COWEN_HOME/.zshrc"
touch "$MOCK_ZSHRC"
export ZDOTDIR="$COWEN_HOME" # Tell the tool where to find .zshrc if it uses it
# Note: Cowen might look at $HOME/.zshrc directly, so we need to be careful.
# I'll check if it respects an environment variable for the RC path.

# 2. Test script generation
echo -e "${BOLD}2. Test Script Generation (Zsh)${NC}"
SCRIPT=$("$COWEN_BIN" completion zsh)
if [[ -n "$SCRIPT" ]] && [[ "$SCRIPT" == *"compdef"* ]]; then
    echo -e "   ${GREEN}✓${NC} Zsh completion script generated successfully"
else
    fail_suite "Failed to generate Zsh completion script"
fi

# 3. Test Install (Mocking HOME for safety)
echo -e "${BOLD}3. Test Installation (--install)${NC}"
# Use a subshell and point HOME to COWEN_HOME so it doesn't mess with the user's real .zshrc
export HOME="$COWEN_HOME"
# Explicitly specify zsh to avoid auto-detection issues in different environments
"$COWEN_BIN" completion --install zsh > /dev/null 2>&1

if grep -q "cowen completion" "$HOME/.zshrc" || grep -q "source" "$HOME/.zshrc"; then
    echo -e "   ${GREEN}✓${NC} Completion installed in .zshrc"
else
    cat "$HOME/.zshrc"
    fail_suite "Completion NOT found in .zshrc"
fi

# 4. Test Uninstall
echo -e "${BOLD}4. Test Uninstallation (--uninstall)${NC}"
"$COWEN_BIN" completion --uninstall zsh > /dev/null 2>&1

if ! grep -q "cowen completion" "$HOME/.zshrc" 2>/dev/null; then
    echo -e "   ${GREEN}✓${NC} Completion uninstalled from .zshrc"
else
    grep "cowen completion" "$HOME/.zshrc"
    fail_suite "Completion still remains in .zshrc"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 23 Passed!${NC}"
