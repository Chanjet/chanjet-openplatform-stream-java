#!/bin/bash
set -e
# Case 54: Config Autonomy (Identifier Locator & Array Collapsing)
# Verifies:
#   1. Identifier Locator (plugins.name:p1)
#   2. Array Append (+)
#   3. Array Unset & Collapsing

# Find common.sh
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source "tests/e2e/scripts/common.sh"
elif [ -f "./cli/cowen/tests/e2e/scripts/common.sh" ]; then
    source "./cli/cowen/tests/e2e/scripts/common.sh"
else
    fail_suite "common.sh not found"
fi

setup_workspace "case_54"

echo "1. Initialize Config with Plugins"
"$COWEN_BIN" config set search.plugins '[{"name":"p1","path":"/path1","type":"search"},{"name":"p2","path":"/path2","type":"search"}]'
"$COWEN_BIN" config set search.enabled '["p1"]'

echo "2. Test Identifier Locator (Set)"
"$COWEN_BIN" config set search.plugins.name:p1.path "/new/path1"
VAL=$("$COWEN_BIN" config get search.plugins.0.path | tr -d '"')
if [ "$VAL" == "/new/path1" ]; then
    echo -e "   ${GREEN}✓${NC} Identifier locator (set) worked"
else
    fail_suite "Identifier locator failed: expected /new/path1, got $VAL"
fi

echo "3. Test Append Mode (+)"
"$COWEN_BIN" config set search.plugins.+ '{"name":"p3","path":"/path3","type":"search"}'
LEN=$("$COWEN_BIN" config get search.plugins --format json | python3 -c "import sys, json; print(len(json.load(sys.stdin)))")
if [ "$LEN" -eq 3 ]; then
    echo -e "   ${GREEN}✓${NC} Append mode worked"
else
    fail_suite "Append mode failed (count: $LEN)"
fi

echo "4. Test Unset & Collapsing"
"$COWEN_BIN" config unset search.plugins.name:p2
# p1 is 0, p2 was 1, p3 was 2. After unset p2, p3 should become 1.
P3_PATH=$("$COWEN_BIN" config get search.plugins.1.path | tr -d '"')
if [ "$P3_PATH" == "/path3" ]; then
    echo -e "   ${GREEN}✓${NC} Array collapsing worked"
else
    fail_suite "Array collapsing failed: got $P3_PATH"
fi

echo "5. Test Immediate Binding"
"$COWEN_BIN" config set search.plugins.name:p1.name "p_new"
# locator name:p1 should now fail
if "$COWEN_BIN" config get search.plugins.name:p1 2>/dev/null; then
    fail_suite "Old locator still worked after rename"
else
    echo -e "   ${GREEN}✓${NC} Immediate binding worked (old locator invalidated)"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 54 Passed!${NC}"
