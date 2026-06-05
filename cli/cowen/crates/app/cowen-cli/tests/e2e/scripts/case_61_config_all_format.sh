#!/bin/bash
set -e
if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_61"
trap cleanup_suite EXIT

echo -e "${BOLD}1. Initialize profiles${NC}"
"$COWEN_BIN" init --profile p1 --app-key test1 --app-secret testsecret --encrypt-key 1234567890123456 --certificate testcert --webhook-target http://localhost/cb --app-mode self-built > /dev/null
"$COWEN_BIN" init --profile p2 --app-key test2 --app-secret testsecret2 --encrypt-key 1234567890123456 --certificate testcert2 --webhook-target http://localhost/cb2 --app-mode self-built > /dev/null

echo -e "${BOLD}2. Test 'cowen config --all' text format${NC}"
output=$("$COWEN_BIN" config --all)

if ! echo "$output" | grep -q "global:"; then
    fail_suite "Missing Global Configuration in --all text output"
fi

if ! echo "$output" | grep -q "p1:"; then
    fail_suite "Missing p1 in --all text output"
fi

if ! echo "$output" | grep -q "p2:"; then
    fail_suite "Missing p2 in --all text output"
fi

echo -e "${BOLD}3. Test 'cowen config --all -o json'${NC}"
json_output=$("$COWEN_BIN" config --all -o json)

if ! echo "$json_output" | jq -e '.global.log.level' > /dev/null; then
    fail_suite "Missing global config in JSON output"
fi

if ! echo "$json_output" | jq -e '.profiles.p1.app_key == "test1"' > /dev/null; then
    fail_suite "Missing or incorrect p1 config in JSON output"
fi

if ! echo "$json_output" | jq -e '.profiles.p2.app_key == "test2"' > /dev/null; then
    fail_suite "Missing or incorrect p2 config in JSON output"
fi

echo -e "${BOLD}4. Test 'cowen config --all -o yaml'${NC}"
yaml_output=$("$COWEN_BIN" config --all -o yaml)

if ! echo "$yaml_output" | grep -q "global:"; then
    fail_suite "Missing global config in YAML output"
fi

if ! echo "$yaml_output" | grep -q "profiles:"; then
    fail_suite "Missing profiles in YAML output"
fi

if ! echo "$yaml_output" | grep -q "p1:"; then
    fail_suite "Missing p1 in YAML output"
fi

if ! echo "$yaml_output" | grep -q "p2:"; then
    fail_suite "Missing p2 in YAML output"
fi

pass_suite
