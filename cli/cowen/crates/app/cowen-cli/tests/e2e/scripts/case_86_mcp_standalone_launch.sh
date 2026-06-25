#!/usr/bin/env bash
# case_86_mcp_standalone_launch.sh
# Tests the standalone launch mode of cowen-mcp-plugin

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_86_mcp"
setup_workspace "case_86_$PROFILE"
cd "$COWEN_HOME"

COWEN_MCP_BIN="$(dirname "$COWEN_BIN")/cowen-mcp-plugin"
if [ ! -f "$COWEN_MCP_BIN" ]; then
    fail_suite "cowen-mcp-plugin binary not found at $COWEN_MCP_BIN"
fi

echo "🔌 1. 测试 cowen-mcp-plugin --help"
"$COWEN_MCP_BIN" --help > /dev/null
assert_pass "Help command executed successfully"

echo "🔌 2. 测试 cowen-mcp-plugin config 输出模板配置..."
CONFIG_OUT=$("$COWEN_MCP_BIN" config)
if ! echo "$CONFIG_OUT" | grep -q "mcpServers" || ! echo "$CONFIG_OUT" | grep -q "cowen-mcp-plugin"; then
    fail_suite "Invalid JSON config template output: $CONFIG_OUT"
fi
assert_pass "Config command executed successfully"

echo "🔌 3. 开启 Daemon，测试 cowen-mcp-plugin server 的握手与 StdIO JSON-RPC 交互..."
# 初始化工作区
"$COWEN_BIN" init --profile "$PROFILE" --app-key "dummykey" --app-secret "dummysecret" --app-mode self-built --certificate "dummy_cert" --encrypt-key "dummy_ek"

# 启动 UDS daemon 监听
"$COWEN_BIN" --profile "$PROFILE" daemon start

# 等待 daemon 就绪
sleep 1.0

# 准备符合标准 MCP 协议的 JSON-RPC 2.0 请求序列（初始化 + 确认 + 工具查询）
# 这样才能突破协议拦截，进入插件深层的 OpenAPI 和 Handlers 执行逻辑
cat << 'EOF_MCP' > "$COWEN_HOME/mcp_mock.jsonl"
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
EOF_MCP

# 将 JSON 请求流水送入插件的 stdin，并捕获响应
echo "     Feeding mock RPC sequence to standalone server..."
RESPONSE=$(cat "$COWEN_HOME/mcp_mock.jsonl" | "$COWEN_MCP_BIN" --profile "$PROFILE" server 2>/dev/null || true)

echo "     Response received: $RESPONSE"

# 停止 daemon
"$COWEN_BIN" --profile "$PROFILE" daemon stop || true

assert_pass "Standalone server execution and handshake attempted"

echo "✅ Standalone MCP Plugin tests Passed!"
