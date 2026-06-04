#!/bin/bash
set -e
# Case 21: OpenAPI Whitelist (Active Interception)
# Verifies:
#   1. 'cowen api' fetches the whitelist from the platform.
#   2. Requests to paths NOT in the whitelist are blocked locally.
#   3. Requests to whitelisted paths are forwarded.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_21"
start_mock

"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_WHITELIST \
    --app-secret AS_WHITELIST \
    --certificate CERT_WHITELIST \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS

assert_pass "SelfBuilt initialized"
sleep 2

# 2. Test Whitelisted Path
echo -e "${BOLD}2. Test Whitelisted Path (/v1/app/data/get)${NC}"
# Use a path that IS in the mock server's spec
"$COWEN_BIN" api POST /v1/app/data/get --data '{"id": 1}' --profile main > "$COWEN_HOME/api_out_1.json" 2>&1
if grep -q "mock_at_sb_" "$COWEN_HOME/api_out_1.json" || grep -q "success" "$COWEN_HOME/api_out_1.json"; then
    echo -e "   ${GREEN}✓${NC} Whitelisted path allowed"
else
    cat "$COWEN_HOME/api_out_1.json"
    fail_suite "Whitelisted path blocked or failed"
fi

# 3. Test Non-Whitelisted Path
echo -e "${BOLD}3. Test Non-Whitelisted Path (/v1/evil/hacker/path)${NC}"
# Use a path that is NOT in the mock server's spec
"$COWEN_BIN" api POST /v1/evil/hacker/path --data '{"cmd": "rm -rf /"}' --profile main > "$COWEN_HOME/api_out_2.json" 2>&1 || true

if grep -i "not in whitelist" "$COWEN_HOME/api_out_2.json" || grep -i "blocked" "$COWEN_HOME/api_out_2.json" || grep -i "Forbidden" "$COWEN_HOME/api_out_2.json" || grep -i "not found in OpenAPI spec" "$COWEN_HOME/api_out_2.json"; then
    echo -e "   ${GREEN}✓${NC} Non-whitelisted path correctly blocked"
else
    cat "$COWEN_HOME/api_out_2.json"
    fail_suite "Non-whitelisted path was NOT blocked"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile main 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 21 Passed!${NC}"
