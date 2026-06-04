#!/bin/bash
# -----------------------------------------------------------------------------
# Test Case 64: Dangling IPC Port Recovery
# -----------------------------------------------------------------------------

set -e

source "$(dirname "$0")/common.sh"
setup_workspace "case_64_dangling_ipc_port"

echo "  [Test A] Simulating a dangling IPC port occupied by another app..."
FAKE_PORT=$(get_unused_port)

python3 -c "
import socket, time, sys
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('127.0.0.1', $FAKE_PORT))
s.listen(1)
open('$COWEN_HOME/fake_server.ready', 'w').close()
try:
    conn, addr = s.accept()
    time.sleep(40)
except:
    pass
" &
FAKE_SERVER_PID=$!

while [ ! -f "$COWEN_HOME/fake_server.ready" ]; do
    sleep 0.1
done

echo "$FAKE_PORT" > "$COWEN_HOME/ipc.port"

echo "  Running cowen status..."
# Run in background and wait with a timeout loop
"$COWEN_BIN" status > "$COWEN_HOME/status.out" 2>&1 &
STATUS_PID=$!

WAIT_SECS=0
TIMED_OUT=0
while kill -0 $STATUS_PID 2>/dev/null; do
    sleep 1
    WAIT_SECS=$((WAIT_SECS + 1))
    if [ $WAIT_SECS -ge 5 ]; then
        TIMED_OUT=1
        kill -9 $STATUS_PID 2>/dev/null || true
        break
    fi
done

echo "OUT:"
cat "$COWEN_HOME/status.out"

if [ $TIMED_OUT -eq 1 ]; then
    echo "❌ cowen status timed out after 5s! Bug is present."
    kill -9 $FAKE_SERVER_PID 2>/dev/null || true
    fail_suite "cowen status hung and timed out."
fi

echo "✅ cowen status returned quickly! (Auto-recovered or handled correctly)"

kill -9 $FAKE_SERVER_PID 2>/dev/null || true
echo -e "\n🎊 Case 64 Passed!\n"
exit 0
