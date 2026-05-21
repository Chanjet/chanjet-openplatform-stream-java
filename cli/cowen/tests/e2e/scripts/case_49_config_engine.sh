#!/bin/bash
# E2E Test: Phase 1 Configuration Engine (Case 49)
# Reference: cli/cowen/docs/WBS.md

if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

# Setup: Isolated environment
setup_workspace "config_engine"

fail() {
    echo -e "${RED}FAILED: $1${NC}"
    exit 1
}

echo "--- Test 1: Set/Get Nested Config ---"
"$COWEN_BIN" config set proxy_port 16001
val=$("$COWEN_BIN" config get proxy_port)
if [ "$val" != "16001" ]; then
    fail "proxy_port mismatch: expected 16001, got '$val'"
fi

"$COWEN_BIN" config set log.level debug
val=$("$COWEN_BIN" config get log.level | tr -d '"')
if [ "$val" != "debug" ]; then
    fail "log.level mismatch: expected debug, got '$val'"
fi

echo "--- Test 2: Global Field Routing ---"
"$COWEN_BIN" config set monitor_port 9090
val=$("$COWEN_BIN" config get monitor_port)
if [ "$val" != "9090" ]; then
    fail "monitor_port mismatch: expected 9090, got '$val'"
fi
# Check app.yaml contains monitor_port
grep -q "monitor_port: 9090" "$COWEN_HOME/app.yaml" || fail "monitor_port not in app.yaml"

echo "--- Test 3: Validation (Interceptors) ---"
"$COWEN_BIN" config set proxy_port 80 && fail "Should have failed for port 80" || echo "PASS: Port validation (80 rejected)"

echo "--- Test 4: Locking (Locked Fields) ---"
"$COWEN_BIN" config set app_key "my-new-key" && fail "Should have failed to lock app_key" || echo "PASS: Locking (app_key protected)"

echo "--- Test 5: Data Masking ---"
"$COWEN_BIN" config set storage.store local
"$COWEN_BIN" config set storage.db_url "sqlite://$COWEN_HOME/db.sqlite"
list_out=$("$COWEN_BIN" config list)
echo "LIST OUT:"
echo "$list_out"
echo "----------"
echo "$list_out" | grep -Fq "db_url: '******'" || fail "db_url not masked"

echo "--- ALL CONFIG ENGINE TESTS PASSED ---"
echo "Passed!"
cleanup_suite
