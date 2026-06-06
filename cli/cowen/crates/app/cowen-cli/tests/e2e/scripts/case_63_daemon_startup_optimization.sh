#!/bin/bash
source "$(dirname "$0")/common.sh"
setup_workspace "case_63_daemon_startup_optimization"
unset COWEN_MONITOR_PORT
DUMMY_PORT=$(get_unused_port)

start_mock
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" \
    --openapi-url $MOCK_URL \
    --stream-url $MOCK_WS >/dev/null

"$COWEN_BIN" config set monitor_port $DUMMY_PORT --global >/dev/null
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
    PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")
    kill -9 "$PID" 2>/dev/null || true
    rm -f "$COWEN_HOME/master_daemon.pid" || true
fi
rm -f "$COWEN_HOME/cowen-ipc.sock" || true
sleep 1

echo "  [Test A] Pre-flight Check & Dynamic Port Recovery"
unset COWEN_ALLOW_PORT_FALLBACK
echo "  Occupying monitor_port $DUMMY_PORT with a dummy process..."
python3 -c "
import socket, time
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('127.0.0.1', $DUMMY_PORT))
s.listen(1)
time.sleep(15)
" &
DUMMY_PID=$!
sleep 1

echo "  Triggering cowen auth login to trigger daemon start and check error output..."
OUT=$("$COWEN_BIN" auth login --profile main --force 2>&1 || true)

if ! cat "$COWEN_HOME/master_daemon.pid" 2>/dev/null | grep -q "Monitor server failed to start"; then
    kill -9 $DUMMY_PID || true
    fail_suite "Expected error message about port occupation. Output:\n$OUT"
fi

kill -9 $DUMMY_PID || true
echo -e "  ${GREEN}✓${NC} Pre-flight check correctly aborted daemon startup when port $DUMMY_PORT was occupied by 3rd party process."
"$COWEN_BIN" daemon stop --all >/dev/null 2>&1 || true
if [ -f "$COWEN_HOME/master_daemon.pid" ]; then
    PID=$(head -n 1 "$COWEN_HOME/master_daemon.pid")
    kill -9 "$PID" 2>/dev/null || true
    rm -f "$COWEN_HOME/master_daemon.pid" || true
fi
rm -f "$COWEN_HOME/cowen-ipc.sock" || true
sleep 1

echo "  [Test B] Synchronous Crash Feedback"
echo "  Corrupting DB URL to force a daemon crash on startup..."
# Since app.yaml is corrupt, we expect the daemon process to crash shortly after start.
# Wait, if app.yaml is corrupt, CLI will also fail to read it.
rm -f "$COWEN_HOME/telemetry.db"
mkdir -p "$COWEN_HOME/telemetry.db"

OUT=$("$COWEN_BIN" daemon start --profile main 2>&1 || true)
if ! echo "$OUT" | grep -q "Daemon stderr tail:"; then
    fail_suite "Expected crash output with daemon stderr. Output:\n$OUT"
fi
if ! echo "$OUT" | grep -q "Failed to init telemetry db"; then
    fail_suite "Expected crash output about telemetry db. Output:\n$OUT"
fi

echo -e "  ${GREEN}✓${NC} Daemon crashed and stderr tail was reported synchronously"

rm -rf "$COWEN_HOME/telemetry.db"
echo -e "\n🎊 Case 63 Passed!\n"
exit 0
