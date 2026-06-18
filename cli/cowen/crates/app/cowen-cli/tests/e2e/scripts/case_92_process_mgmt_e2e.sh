#!/usr/bin/env bash
# case_92_process_mgmt_e2e.sh
# Tests cross-platform process management (port occupancy & profile extraction from cmdline).

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROXY_PORT=$(get_unused_port)
setup_workspace "case_92"
trap cleanup_suite EXIT

# 拷贝成标准名称以避开 macOS 15 字符限制和 cmdline 限制
cp "$COWEN_BIN" "$COWEN_HOME/cowen"
cp "$COWEN_DAEMON_BIN" "$COWEN_HOME/cowen-daemon"
export COWEN_BIN="$COWEN_HOME/cowen"
export COWEN_DAEMON_BIN="$COWEN_HOME/cowen-daemon"
chmod +x "$COWEN_BIN" "$COWEN_DAEMON_BIN"


echo "📦 1. 初始化 Profile pm1 并配置其使用代理端口 $PROXY_PORT"
"$COWEN_BIN" init --profile "pm1" \
    --app-key "dummy_key_1" \
    --app-secret "dummy_secret_1" \
    --app-mode self-built \
    --certificate "dummy_cert" \
    --encrypt-key "1234567890123456" \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port "$PROXY_PORT" \
    --webhook-target "http://127.0.0.1:8080/cb"

echo "📦 2. 启动 pm1 的 daemon"
"$COWEN_BIN" daemon start --profile "pm1" >/dev/null
wait_for_daemon "pm1" 10

echo "📦 3. 将 master_daemon.pid 暂时移走，使得 status 命令判定 Daemon 为 OFFLINE"
mv "$COWEN_HOME/master_daemon.pid" "$COWEN_HOME/master_daemon.pid.bak"

echo "📦 4. 查询 pm1 的状态。此时它因为没有 PID 文件判定为 OFFLINE，并触发端口冲突检测，进而从进程命令行提取 Profile"
STATUS_OUT=$(COWEN_SKIP_DAEMON_RECOVERY=true "$COWEN_BIN" status --profile "pm1" 2>&1 || true)
echo "$STATUS_OUT"

# 恢复 master_daemon.pid 以便干净清理
mv "$COWEN_HOME/master_daemon.pid.bak" "$COWEN_HOME/master_daemon.pid"

if ! echo "$STATUS_OUT" | grep -E -q "已被 Profile 'pm1'|已被 Profile 'unknown'"; then
    fail_suite "Failed to detect port conflict with Profile 'pm1' or 'unknown'. Output: $STATUS_OUT"
fi

echo "📦 5. 停止 pm1 的 daemon"
"$COWEN_BIN" daemon stop --profile "pm1" || true
echo -n "   Waiting for proxy port $PROXY_PORT to be released..."
for i in {1..30}; do
    if ! lsof -i :$PROXY_PORT >/dev/null 2>&1; then
        echo " [FREE]"
        break
    fi
    echo -n "."
    sleep 0.5
done

echo "📦 6. 使用 python3 监听该端口，模拟非 cowen 进程占用"
python3 -c "
import socket, time
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(('127.0.0.1', $PROXY_PORT))
    s.listen(1)
    print('Python listener ready on port $PROXY_PORT')
    time.sleep(5)
except Exception as e:
    print('Python listener failed:', e)
" &
PY_PID=$!
sleep 1.5

echo "📦 7. 再次查询 pm1 的状态，检测端口被 non-cowen 进程占用"
mv "$COWEN_HOME/master_daemon.pid" "$COWEN_HOME/master_daemon.pid.bak"
STATUS_OUT2=$("$COWEN_BIN" status --profile "pm1" 2>&1 || true)
echo "$STATUS_OUT2"
mv "$COWEN_HOME/master_daemon.pid.bak" "$COWEN_HOME/master_daemon.pid"

# 杀死 Python 进程以释放端口
kill "$PY_PID" 2>/dev/null || true

if ! echo "$STATUS_OUT2" | grep -E -q "已被进程 '.*' \(PID: [0-9]+\) 占用|已被进程 'python3'|已被进程 'Unknown Process'"; then
    fail_suite "Failed to detect port conflict with unknown process. Output: $STATUS_OUT2"
fi

assert_pass "Process management port occupancy tested successfully"
echo "✅ Process management E2E tests Passed!"
