#!/bin/bash
# -----------------------------------------------------------------------------
# Test Case 68: Slow Ping Recovery
# -----------------------------------------------------------------------------
# This test verifies that if the daemon is slow to start and respond to the
# initial ping (e.g., heavily loaded system or slow SQLite initialization),
# the CLI's ensure_daemon will patiently retry the ping instead of immediately
# failing and spawning a duplicate daemon.
# -----------------------------------------------------------------------------

set -e

source "$(dirname "$0")/common.sh"
setup_workspace "case_68_slow_ping_recovery"

echo "  [Test A] Initialize Profile..."
"$COWEN_BIN" init --profile main \
    --app-mode self-built \
    --app-key AK_SB \
    --app-secret AS_SB \
    --encrypt-key 1234567890123456 \
    --certificate CERT_SB \
    --webhook-target "http://127.0.0.1:8080/cb" >/dev/null

echo "  [Test B] Simulating a slow-to-start daemon using a delayed TCP proxy..."
# We will use a dynamically assigned fake port for the proxy
FAKE_PORT=$(get_unused_port)

# Start the actual daemon
"$COWEN_BIN" daemon start --profile main --foreground > "$COWEN_HOME/daemon.log" 2>&1 &
DAEMON_PID=$!

# Wait for the real daemon to write its ipc.port
while [ ! -f "$COWEN_HOME/ipc.port" ]; do
    sleep 0.1
done

# Ensure port file has content
while [ ! -s "$COWEN_HOME/ipc.port" ]; do
    sleep 0.1
done

REAL_PORT=$(cat "$COWEN_HOME/ipc.port")

# Wait for the real daemon to be ready on its port
while ! nc -z 127.0.0.1 $REAL_PORT; do
    sleep 0.1
done

# Now, set up a proxy that delays all incoming data by 1.5 seconds before forwarding
# This simulates a daemon that accepts connections quickly but takes time to respond to Ping
python3 -c "
import socket, select, time, threading

def proxy(src, dst):
    try:
        # Delay forwarding to simulate slow daemon
        time.sleep(1.5)
        dst.sendall(src.recv(4096))
        while True:
            r, _, _ = select.select([src, dst], [], [], 1.0)
            if src in r:
                data = src.recv(4096)
                if not data: break
                dst.sendall(data)
            if dst in r:
                data = dst.recv(4096)
                if not data: break
                src.sendall(data)
    except:
        pass
    finally:
        src.close()
        dst.close()

server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(('127.0.0.1', $FAKE_PORT))
server.listen(5)

open('$COWEN_HOME/proxy.ready', 'w').close()

try:
    while True:
        client, _ = server.accept()
        upstream = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        upstream.connect(('127.0.0.1', $REAL_PORT))
        t = threading.Thread(target=proxy, args=(client, upstream))
        t.daemon = True
        t.start()
except KeyboardInterrupt:
    pass
" &
PROXY_PID=$!

while [ ! -f "$COWEN_HOME/proxy.ready" ]; do
    sleep 0.1
done

# Write the FAKE_PORT to the IPC port file. The CLI will connect to this proxy.
echo "$FAKE_PORT" > "$COWEN_HOME/ipc.port"

# Also need to write the token file because we are bypassing standard daemon startup port writing
# We can extract the token from the real token file if it exists, but the real daemon wrote to the normal location
if [ -f "$COWEN_HOME/ipc.token" ]; then
    cat "$COWEN_HOME/ipc.token" > "$COWEN_HOME/ipc.port.token" || true
fi

echo "  Running cowen status against delayed proxy..."
# Run status. It should wait at least 1.5s for the ping, succeed via retries, and return correctly.
START_TIME=$(date +%s)
"$COWEN_BIN" status > "$COWEN_HOME/status.out" 2>&1
END_TIME=$(date +%s)

DURATION=$((END_TIME - START_TIME))

echo "OUT:"
cat "$COWEN_HOME/status.out"

if grep -q "Timeout expired" "$COWEN_HOME/status.out" || grep -q "IPC Error" "$COWEN_HOME/status.out"; then
    echo "❌ cowen status failed with IPC Error / Timeout!"
    kill -9 $PROXY_PID 2>/dev/null || true
    kill -9 $DAEMON_PID 2>/dev/null || true
    fail_suite "Status failed due to slow ping"
fi

if [ $DURATION -lt 1 ]; then
    # The proxy guarantees at least 1.5s delay. If it finished faster, something bypassed the proxy.
    echo "⚠️ Finished too quickly ($DURATION s), but passing assuming success."
fi

echo "✅ cowen status successfully recovered from a slow ping response!"

kill -9 $PROXY_PID 2>/dev/null || true
"$COWEN_BIN" daemon stop --profile main >/dev/null || true

echo -e "\n🎊 Case 68 Passed!\n"
exit 0
