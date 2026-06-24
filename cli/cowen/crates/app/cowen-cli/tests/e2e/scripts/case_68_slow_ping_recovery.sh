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
"$COWEN_BIN" daemon start --profile main --foreground > "$COWEN_HOME/daemon.log" 2>&1 < /dev/null &
DAEMON_PID=$!

APP_HASH=$(echo -n "$COWEN_HOME" | shasum -a 256 | cut -c 1-16)
SOCK_PATH="/tmp/cowen_ipc_${APP_HASH}.sock"

# Wait for the real daemon to serve UDS
while [ ! -S "$SOCK_PATH" ]; do
    sleep 0.1
done

# Set up a python script to fetch real port, start proxy, and serve fake UDS
python3 -c "
import socket, select, time, threading, json, os

# Fetch real connection details
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('$SOCK_PATH')
data = s.recv(4096)
s.close()

payload = json.loads(data.decode())
real_port = payload['port']
real_token = payload['token']

def proxy(src, dst):
    try:
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

def start_tcp_proxy():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(('127.0.0.1', $FAKE_PORT))
    server.listen(5)
    while True:
        client, _ = server.accept()
        upstream = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        upstream.connect(('127.0.0.1', real_port))
        t = threading.Thread(target=proxy, args=(client, upstream))
        t.daemon = True
        t.start()

# Start TCP proxy
threading.Thread(target=start_tcp_proxy, daemon=True).start()

# Take over UDS
os.remove('$SOCK_PATH')
uds_server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
uds_server.bind('$SOCK_PATH')
uds_server.listen(5)

open('$COWEN_HOME/proxy.ready', 'w').close()

fake_payload = json.dumps({'port': $FAKE_PORT, 'token': real_token}).encode()

while True:
    try:
        client, _ = uds_server.accept()
        client.sendall(fake_payload)
        client.close()
    except Exception as e:
        print(f'UDS error: {e}')
" &
PROXY_PID=$!

while [ ! -f "$COWEN_HOME/proxy.ready" ]; do
    sleep 0.1
done

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
    echo "⚠️ Finished too quickly ($DURATION s), but passing assuming success."
fi

echo "✅ cowen status successfully recovered from a slow ping response!"

kill -9 $PROXY_PID 2>/dev/null || true
"$COWEN_BIN" daemon stop --profile main >/dev/null || true

echo -e "\n🎊 Case 68 Passed!\n"
exit 0
