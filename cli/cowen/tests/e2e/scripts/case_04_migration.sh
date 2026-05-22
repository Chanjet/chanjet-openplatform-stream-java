#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_04"
trap cleanup_suite EXIT

echo -e "${BOLD}1. Source Data (Local Store)${NC}"
"$COWEN_BIN" store set --store local >/dev/null
"$COWEN_BIN" init --profile mig_prof \
    --app-mode self-built \
    --app-key AK_MIG \
    --app-secret AS_MIG \
    --encrypt-key 1234567890123456 \
    --certificate CERT_MIG >/dev/null
assert_pass "Local profile created"

echo -e "${BOLD}2. Migrate to SQLite (InnerDB)${NC}"
# MODE move: copy and delete source
"$COWEN_BIN" store migrate --to "innerdb://$COWEN_HOME/cowen.db" --mode move >/dev/null
assert_pass "Migration finished"

echo -e "${BOLD}3. Integrity Check${NC}"
"$COWEN_BIN" store status | grep -q "innerdb"
assert_pass "Active store is InnerDB"

"$COWEN_BIN" config --profile mig_prof | grep -q "AK_MIG"
assert_pass "Configuration preserved"

if [ -f "$COWEN_HOME/cowen.db" ]; then
    echo -e "  ${GREEN}✓${NC} SQLite DB file exists"
else
    echo -e "  ${RED}✗${NC} SQLite DB file missing"
    exit 1
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile mig_prof 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 04 Passed!${NC}"
