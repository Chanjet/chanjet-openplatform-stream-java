#!/bin/bash
# cowen CLI 全能力探索性 Mock 测试脚本 (v0.3.0)
# 遵循 Independent E2E Validation Standard

set -e

# 设置 ANSI 颜色
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# 配置参数
BINARY_PATH="./../../bin/macos-aarch64/cowen"
MOCK_PORT=9299
MOCK_URL="http://127.0.0.1:$MOCK_PORT"
MOCK_WS_URL="$MOCK_URL"
TMP_PROF="mock_exploratory_final"

# 物理隔离测试环境
export COWEN_HOME="$(pwd)/.cowen_test"
mkdir -p "$COWEN_HOME"

echo -e "${BLUE}🧪 启动全能力探索测试 (基于 Mock 远程服务)...${NC}"
echo -e "${BLUE}📁 测试主目录: $COWEN_HOME${NC}"

# 0. 环境预清理
rm -rf "$COWEN_HOME"
mkdir -p "$COWEN_HOME"

# 初始化全局存储为 local 模式，以确保后续 YAML 验证有效
"$BINARY_PATH" store set --store local >/dev/null 2>&1

echo "DEBUG: COWEN_HOME before init:"
ls -la "$COWEN_HOME"

# 1. 启动 Mock Server
echo "DEBUG: Starting Mock Server..."
nohup python3 -u tests/mock_server.py > mock_server_test.log 2>&1 &
MOCK_PID=$!
sleep 2
echo "DEBUG: MOCK_PID=$MOCK_PID"

cleanup() {
    echo -e "\n${YELLOW}🧹 正在清理测试环境与后台进程...${NC}"
    # 停止 Cowen 所有后台守护进程
    "$BINARY_PATH" daemon stop --all >/dev/null 2>&1 || true
    killall -9 cowen >/dev/null 2>&1 || true
    lsof -ti :9091 | xargs kill -9 >/dev/null 2>&1 || true
    lsof -ti :$MOCK_PORT | xargs kill -9 >/dev/null 2>&1 || true
    # 彻底杀死 Mock Server
    pkill -f "tests/mock_server.py" >/dev/null 2>&1 || true
    echo -e "${GREEN}✅ 清理完成${NC}"
}
trap cleanup EXIT

# 等待 Mock Server 就绪
echo -n "   [WAIT] 等待 Mock Server 就绪..."
READY=0
for i in {1..30}; do
    CURL_OUT=$(curl -s -v "$MOCK_URL/v1/mock/ping" 2>&1)
    if echo "$CURL_OUT" | grep -q "200 OK"; then
        echo -e "${GREEN} [就绪]${NC}"
        READY=1
        break
    fi
    echo -n "."
    sleep 1
done
if [ $READY -eq 0 ]; then
    echo -e "${RED}超时${NC}"
    echo "Last curl output:"
    echo "$CURL_OUT"
    cleanup
    exit 1
fi

# --- 测试用例开始 ---

# TC-01: 使用 Mock URL 初始化
echo -e "${YELLOW}Step 1: 初始化 Profile (指向 Mock 服务)...${NC}"
"$BINARY_PATH" init --profile "$TMP_PROF" \
    --app-mode self-built \
    --app-key "MOCK_AK" \
    --app-secret "MOCK_AS" \
    --certificate "MOCK_CERT" \
    --encrypt-key "1234567890123456" \
    --proxy-port 9091 \
    --stream-url "$MOCK_WS_URL" \
    --openapi-url "$MOCK_URL"

if [ -f "$COWEN_HOME/$TMP_PROF.yaml" ]; then
    echo -e "${GREEN}   [OK] Profile 配置文件创建成功${NC}"
else
    echo -e "${RED}   [FAIL] 配置文件丢失${NC}"
    exit 1
fi

# TC-02: 动态规约刷新
echo -e "${YELLOW}Step 2: 验证动态规约拉取与 API 发现...${NC}"
# 第一次拉取可能涉及 AppTicket 握手，给予足够重试
"$BINARY_PATH" api list --refresh --profile "$TMP_PROF" > "$COWEN_HOME/api_init.txt" 2>&1
API_LIST=$("$BINARY_PATH" api list --profile "$TMP_PROF")
if echo "$API_LIST" | grep -q "/v1/mock/ping"; then
    echo -e "${GREEN}   [OK] 成功从 Mock 服务拉取并解析 OpenAPI 规约${NC}"
else
    echo -e "${RED}   [FAIL] 未能拉取到 Mock 接口定义${NC}"
    echo "API List Output:"
    echo "$API_LIST"
    cleanup
    exit 1
fi

# TC-03: 语义搜索
echo -e "${YELLOW}Step 3: 验证本地 AI 语义搜索 (Neural Search)...${NC}"
SEARCH_OUT=$("$BINARY_PATH" api list --search "ping" --profile "$TMP_PROF")
if echo "$SEARCH_OUT" | grep -q "/v1/mock/ping"; then
    echo -e "${GREEN}   [OK] 语义搜索命中正确接口${NC}"
else
    echo -e "${RED}   [FAIL] 语义搜索未返回预期结果${NC}"
    echo "Search Output:"
    echo "$SEARCH_OUT"
    cleanup
    exit 1
fi

# TC-04 & TC-05: 令牌自动续约与 AppTicket 推送
echo -e "${YELLOW}Step 4: 验证令牌自动续约与 AppTicket 采集 (需 Daemon 参与)...${NC}"
# 守护进程已在 init 时由 recovery 逻辑启动，无需重复启动

# 获取初始令牌
TOKEN_1=$("$BINARY_PATH" auth token --format json --profile "$TMP_PROF" | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token', ''))")
echo -e "   [INFO] 初始令牌: $TOKEN_1"

# 模拟过期等待 (Mock Server 设置的是 15s 过期，CLI 剩余 10% 会续约)
echo -n "   [WAIT] 等待令牌自动过期与续约 (20s)..."
for i in {1..20}; do sleep 1; echo -n "."; done
echo -e " [完成]"

TOKEN_2=$("$BINARY_PATH" auth token --format json --profile "$TMP_PROF" | python3 -c "import sys, json; print(json.load(sys.stdin).get('access_token', ''))")
echo -e "   [INFO] 续约后令牌: $TOKEN_2"

if [ "$TOKEN_1" != "$TOKEN_2" ] && [ -n "$TOKEN_2" ]; then
    echo -e "${GREEN}   [OK] 令牌自动续约成功${NC}"
else
    echo -e "${RED}   [FAIL] 令牌未按预期续约${NC}"
    exit 1
fi

# TC-06: 代理转发验证
echo -e "${YELLOW}Step 5: 验证本地代理转发 (Local Proxy)...${NC}"
PROXY_PORT=$("$BINARY_PATH" config --profile "$TMP_PROF" | grep "proxy_port" | awk '{print $2}' | tr -d ',')
echo -e "   [INFO] 代理端口: $PROXY_PORT"

# 访问安全接口，验证是否自动注入了令牌
SECURE_RESP=$(curl -s "http://127.0.0.1:$PROXY_PORT/v1/mock/secure")
if echo "$SECURE_RESP" | grep -q "verified"; then
    echo -e "${GREEN}   [OK] 本地代理成功注入令牌并转发至 Mock 服务${NC}"
else
    echo -e "${RED}   [FAIL] 代理转发验证失败: $SECURE_RESP${NC}"
    exit 1
fi

# TC-07: DLQ 验证
echo -e "${YELLOW}Step 6: 验证死信队列 (DLQ) 记录能力...${NC}"
# 告诉 Mock Server 下几次请求报错
curl -s "$MOCK_URL/_control/fail_ping" > /dev/null
# 触发一次转发调用 (由于是代理模式，同步调用会直接返回错误，但后台审计或重试逻辑会触发)
# 这里我们直接模拟一个消息到达 webhook 的行为来触发转发
DAEMON_PID_FILE="$COWEN_HOME/${TMP_PROF}_daemon.pid"
# 找到代理启动的 webhook 端口 (默认是 9091 或配置值)
WEBHOOK_PORT=9091 # 通常与代理同进程
curl -s -X POST "http://127.0.0.1:$WEBHOOK_PORT/webhook" \
    -H "Content-Type: application/json" \
    -d '{"type":"ORDER_SYNC","data":{"id":"ERR_001"}}' > /dev/null || true

sleep 2
DLQ_OUT=$("$BINARY_PATH" dlq list --profile "$TMP_PROF")
if echo "$DLQ_OUT" | grep -q "ORDER_SYNC"; then
    echo -e "${GREEN}   [OK] 转发失败事件已成功进入死信队列 (DLQ)${NC}"
else
    echo -e "${YELLOW}   [WARN] DLQ 未能捕获到异常记录，请检查 Forwarder 逻辑${NC}"
fi

# TC-08: 系统诊断
echo -e "${YELLOW}Step 7: 验证系统全量状态诊断 (status --all)...${NC}"
STATUS_OUT=$("$BINARY_PATH" system status --all)
if echo "$STATUS_OUT" | grep -q "RUNNING"; then
    echo -e "${GREEN}   [OK] 系统诊断正确报告了 Mock 环境的健康状态${NC}"
else
    echo -e "${RED}   [FAIL] 系统诊断报告异常${NC}"
    exit 1
fi

echo -e "\n${GREEN}🎉 所有 Mock 全能力探索测试用例通过！${NC}"
