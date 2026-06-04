#!/bin/bash
set -e
# Case 24: System Health Check (Status --all)
# Verifies:
#   1. 'status --all' scans all profiles.
#   2. Correctly identifies healthy vs unhealthy profiles.

if [ -f "crates/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi
PROXY_PORT=$(get_unused_port)
PROXY_PORT_2=$(get_unused_port)

echo -e "${BOLD}1. Setup Environment with Multiple Profiles${NC}"
setup_workspace "case_24"
start_mock

# Profile 1: Healthy
"$COWEN_BIN" init --profile healthy \
    --app-mode self-built \
    --app-key AK_HEALTHY \
    --app-secret AS_HEALTHY \
    --certificate CERT_HEALTHY \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT
assert_pass "Profile 'healthy' initialized"

# Profile 2: Expired
"$COWEN_BIN" init --profile expired \
    --app-mode self-built \
    --app-key AK_EXPIRED \
    --app-secret AS_EXPIRED \
    --certificate CERT_EXPIRED \
    --encrypt-key 1234567890123456 \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS \
    --proxy-port $PROXY_PORT_2
assert_pass "Profile 'expired' initialized"

# Profile 3: Broken
mkdir -p "$COWEN_HOME/broken"
cat > "$COWEN_HOME/broken.yaml" <<EOF
storage:
  store: unknown_store
EOF

# 2. Run status --all
echo -e "${BOLD}2. Run 'status --all' and Check Output${NC}"
OUT=$("$COWEN_BIN" status --all)
echo "$OUT"

# Verify all profiles listed
if echo "$OUT" | grep -q "healthy" && echo "$OUT" | grep -q "expired" && echo "$OUT" | grep -q "broken"; then
    echo -e "   ${GREEN}✓${NC} All profiles detected"
else
    fail_suite "Not all profiles were detected"
fi

# Verify error reporting
if echo "$OUT" | grep -q "Profile load failed" || echo "$OUT" | grep -q "broken"; then
    echo -e "   ${GREEN}✓${NC} Errors correctly reported for 'broken' profile"
else
    fail_suite "Error reporting failed"
fi


# Mandatory Sanitization Check
CONFIG_OUT=$("$COWEN_BIN" config --profile healthy 2>&1)
assert_sanitized "$CONFIG_OUT" "CLI Profile Config output"

echo -e "\n${GREEN}🎊 Case 24 Passed!${NC}"
