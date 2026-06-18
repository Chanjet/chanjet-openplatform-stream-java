#!/usr/bin/env bash
# case_90_wasm_plugin_pipeline.sh
# Tests Wasm plugin loading, route mapping, request header filtering, and host callback.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROXY_PORT=$(get_unused_port)
setup_workspace "case_90"
trap cleanup_suite EXIT
start_mock

PROFILE="wasm_test"

# 1. 编译或寻找 WASM 插件
WASM_SRC=""
CANDIDATES=(
    "target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm"
    "target/wasm32-wasip1/debug/cowen_wasm_auth_selfbuilt.wasm"
    "target/release/cowen_wasm_auth_selfbuilt.wasm"
    "target/debug/cowen_wasm_auth_selfbuilt.wasm"
    "$HOME/.cowen/system_plugins/cowen_wasm_auth_selfbuilt.wasm"
)

for c in "${CANDIDATES[@]}"; do
    if [ -f "$c" ]; then
        WASM_SRC="$c"
        break
    fi
done

if [ -z "$WASM_SRC" ]; then
    echo "⚠️  Wasm plugin source not found, compiling manually..."
    rustup target add wasm32-wasip1 || true
    cargo build --release -p cowen-wasm-auth-selfbuilt --target wasm32-wasip1
    WASM_SRC="target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm"
fi

echo "📦 1. 物理配置 Wasm 插件及其管道配置..."
mkdir -p "$COWEN_HOME/plugins"
cp "$WASM_SRC" "$COWEN_HOME/plugins/cowen_wasm_auth_selfbuilt.wasm"
cp "crates/plugins/cowen-wasm-auth-selfbuilt/plugin.json" "$COWEN_HOME/plugins/cowen-wasm-auth-selfbuilt.json"

cat > "$COWEN_HOME/plugins/pipeline.yaml" <<EOF
plugins:
  - name: cowen-wasm-auth-selfbuilt
    path: $COWEN_HOME/plugins/cowen_wasm_auth_selfbuilt.wasm
routes:
  - path_prefix: /api/
    pre_auth_plugins: []
    request_filter_plugins:
      - cowen-wasm-auth-selfbuilt
    response_filter_plugins: []
EOF

echo "📦 2. 初始化 profile 并写入 Mock 凭据..."
"$COWEN_BIN" init --profile "$PROFILE" \
    --app-key "dummy_app_key" \
    --app-secret "dummy_app_secret" \
    --app-mode self-built \
    --certificate "dummy_cert" \
    --encrypt-key "1234567890123456" \
    --openapi-url "$MOCK_URL" \
    --stream-url "$MOCK_WS" \
    --proxy-port "$PROXY_PORT" \
    --webhook-target "http://127.0.0.1:8080/cb"

# 写入 mock access token 供 Facade 读取
sqlite3 "$COWEN_HOME/cowen.db" <<EOF
INSERT OR REPLACE INTO cowen_token (profile, item_key, item_value, expires_at)
VALUES ('$PROFILE', 'access', '{"value":"wasm_mocked_token","expires_at":"2099-01-01T00:00:00Z","created_at":"2026-01-01T00:00:00Z"}', 4070880000);
INSERT OR REPLACE INTO cowen_app_token (app_key, token_value, expires_at, created_at)
VALUES ('dummy_app_key', 'wasm_mocked_token', '2099-01-01 00:00:00', '2026-01-01 00:00:00');
EOF

echo "📦 3. 启动守护进程并等待代理端口就绪..."
"$COWEN_BIN" daemon start --profile "$PROFILE" >/dev/null
wait_for_daemon "$PROFILE" 10

echo -n "   Waiting for proxy port $PROXY_PORT to be bound..."
PORT_READY=0
for i in {1..20}; do
    if lsof -i :$PROXY_PORT >/dev/null 2>&1; then
        PORT_READY=1
        echo " [READY]"
        break
    fi
    echo -n "."
    sleep 0.5
done

if [ "$PORT_READY" -ne 1 ]; then
    fail_suite "Proxy port $PROXY_PORT did not start listening in time."
fi

echo "📦 4. 通过代理访问接口触发 Wasm 过滤..."
curl -s -X POST "$MOCK_URL/control/reset" >/dev/null || true

RESPONSE=$(curl -s -X POST "http://127.0.0.1:$PROXY_PORT/v1/app/data/get")
echo "Response: $RESPONSE"

echo "📦 5. 校验 Mock 服务端是否收到了注入的 Headers..."
if ! echo "$RESPONSE" | grep -q -E "wasm_mocked_token|mock_at_sb_"; then
    fail_suite "Wasm plugin did not inject openToken into the request. Response: $RESPONSE"
fi

if ! echo "$RESPONSE" | grep -q "dummy_app_key"; then
    fail_suite "Wasm plugin did not inject appKey into the request. Response: $RESPONSE"
fi

kill_daemons_in_dirs "$COWEN_HOME"

assert_pass "Wasm plugin pipeline tested successfully"
echo "✅ Wasm plugin pipeline E2E tests Passed!"
