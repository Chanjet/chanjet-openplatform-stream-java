#!/bin/bash
set -e
if [ -f "tests/e2e/scripts/common.sh" ]; then
    source tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_60"
trap cleanup_suite EXIT

echo -e "${BOLD}1. Setup Dummy Process on a Dynamic Port${NC}"
TEST_PORT=$(get_unused_port)
echo "Using port $TEST_PORT"

cat << EOF > "$COWEN_HOME/test_bind.py"
import socket
import time
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(('127.0.0.1', $TEST_PORT))
    s.listen(1)
    print("Dummy process bound to $TEST_PORT")
    time.sleep(30)
except Exception as e:
    print(f"Dummy process failed: {e}")
EOF
python3 "$COWEN_HOME/test_bind.py" &
PY_PID=$!
sleep 1

echo -e "${BOLD}2. Initialization${NC}"
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" >/dev/null
assert_pass "Profile initialized"

# Reset monitor_port to 0 to test fallback logic
"$COWEN_BIN" config set monitor_port 0 --global >/dev/null
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
    PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")
    kill -9 "$PID" 2>/dev/null || true
    rm -f "$COWEN_HOME/master_daemon.pid" || true
fi
rm -f "$COWEN_HOME/ipc.port" || true
rm -f "$COWEN_HOME/ipc.token" || true
sleep 1

# Verify initial config is 0
INIT_PORT=$("$COWEN_BIN" config -o json | grep '"monitor_port"' | awk -F': ' '{print $2}' | tr -d ',')
if [ "$INIT_PORT" != "0" ]; then
    fail_suite "Default monitor_port should be 0, got $INIT_PORT"
fi

echo -e "${BOLD}3. Daemon Startup with Fallback (First time)${NC}"
"$COWEN_BIN" daemon start --profile main >/dev/null
assert_pass "Daemon start command sent"

wait_for_daemon main 10
assert_pass "Daemon is running and healthy"

echo -e "${BOLD}4. Verify Monitor Port in Config${NC}"
ACTUAL_PORT=$("$COWEN_BIN" config -o json | grep '"monitor_port"' | awk -F': ' '{print $2}' | tr -d ',')

if [ "$ACTUAL_PORT" == "$TEST_PORT" ]; then
    kill $PY_PID || true
    fail_suite "Monitor port did not fallback, still $TEST_PORT"
elif [ -z "$ACTUAL_PORT" ] || [ "$ACTUAL_PORT" == "0" ]; then
    kill $PY_PID || true
    fail_suite "Monitor port was not saved correctly (got '$ACTUAL_PORT')"
else
    echo -e "  ${GREEN}✓${NC} Monitor port successfully fell back to random port: $ACTUAL_PORT"
fi

echo -e "${BOLD}5. Stop Daemon and Reset Config to Explicit $TEST_PORT${NC}"
"$COWEN_BIN" daemon stop --all >/dev/null || true
if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
    PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")
    kill -9 "$PID" 2>/dev/null || true
    rm -f "$COWEN_HOME/master_daemon.pid" || true
fi
rm -f "$COWEN_HOME/cowen-ipc.sock" || true
sleep 1

# Manually set monitor_port to $TEST_PORT to simulate non-first time
"$COWEN_BIN" config set monitor_port $TEST_PORT --global >/dev/null

echo -e "${BOLD}6. Daemon Startup without Fallback (Non-first time)${NC}"
# The daemon should fail because $TEST_PORT is occupied and fallback is not allowed
"$COWEN_BIN" daemon start --profile main >/dev/null 2>&1 || true
sleep 2

# Check if daemon is running. It should NOT be running.
if "$COWEN_BIN" status --profile main | grep -q "RUNNING"; then
    kill $PY_PID || true
    fail_suite "Daemon started successfully on explicit $TEST_PORT, it should have failed!"
else
    echo -e "  ${GREEN}✓${NC} Daemon correctly failed to start when port $TEST_PORT was explicitly occupied."
fi

kill $PY_PID || true
pass_suite
