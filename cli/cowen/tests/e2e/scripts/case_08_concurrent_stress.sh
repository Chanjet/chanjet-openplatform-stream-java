#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh

PROXY_PORT=$(get_unused_port)
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "concurrency"
trap cleanup_suite EXIT
start_mock

echo -e "${BOLD}1. Initialization${NC}"
"$COWEN_BIN" init --profile stress --app-mode self-built \
    --app-key AK_STRESS --app-secret AS_STRESS --encrypt-key 1234567890123456 --certificate CERT_STRESS \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS --proxy-port $PROXY_PORT >/dev/null
assert_pass "Profile initialized"

echo -e "${BOLD}2. Start Daemon${NC}"
"$COWEN_BIN" daemon start --profile stress >/dev/null
sleep 2
assert_pass "Daemon is running"

echo -e "${BOLD}3. Concurrent Proxy Requests (Stress)${NC}"
echo "   Launching 20 parallel requests..."
PIDS=()
for i in {1..20}; do
    curl -s http://127.0.0.1:$PROXY_PORT/v1/mock/ping >/dev/null &
    PIDS+=($!)
done

# Wait for all
for pid in "${PIDS[@]}"; do
    wait $pid
done
assert_pass "All concurrent requests finished without crash"

echo -e "${BOLD}4. Audit Log Check${NC}"
# Allow daemon async task to flush vault sqlite writes
sleep 2

# Verify audit log shows successful proxying
"$COWEN_BIN" log view audit --profile stress -n 50 | grep -q "Request successfully proxied"
assert_pass "Audit log verified"

echo -e "\n${GREEN}🎊 Case 08 Passed!${NC}"
