#!/bin/bash

# 畅捷通 Stream Gateway 全链路回归测试脚本
# 包含：Java单元测试、Node.js单元测试、集成转发验证

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "===================================================="
echo "🚀 开始执行全链路回归测试"
echo "===================================================="

# 1. 环境准备
export JAVA_HOME="/Users/zhangliang/Library/Java/JavaVirtualMachines/graalvm-jdk-21.0.8/Contents/Home"
export PATH="$JAVA_HOME/bin:$PATH"
echo "📍 使用 JDK: $JAVA_HOME"

# 2. 编译并执行 Java 单元测试
echo -e "\n🔍 [Step 1/4] 执行 Java 单元测试..."
make test > /dev/null
echo -e "${GREEN}✅ Java 单元测试全部通过${NC}"

# 3. 执行 Node.js SDK 单元测试
echo -e "\n🔍 [Step 2/4] 执行 Node.js SDK 单元测试..."
cd sdk/nodejs
npm run build > /dev/null
npm test > /dev/null
cd ../..
echo -e "${GREEN}✅ Node.js SDK 单元测试全部通过${NC}"

# 4. 集成验证 (Webhook -> Gateway -> Node.js SDK)
echo -e "\n🔍 [Step 3/4] 启动集成环境进行全链路验证..."

# 确保 8080 端口空闲
lsof -i :8080 -t | xargs kill -9 2>/dev/null || true

# 启动网关 (使用 localhost 模拟模式)
nohup java -jar services/gateway-java/connector-server/target/connector-server.jar --spring.profiles.active=localhost > gateway_full_test.log 2>&1 &
GW_PID=$!

echo "⏳ 等待网关启动 (15s)..."
sleep 15

# 启动 Node.js Demo 客户端
cd sdk/nodejs-demo
npm install > /dev/null
nohup node main.js > demo_full_test.log 2>&1 &
DEMO_PID=$!

echo "⏳ 等待客户端连接 (5s)..."
sleep 5

# 5. 触发 Webhook 测试
echo "📡 发送明文消息验证..."
curl -s -X POST http://localhost:8080/internal/v1/webhook/dispatch \
  -H "X-C-APP_KEY: 3qMYSkA5" \
  -H "X-MSG-ID: msg-plain-001" \
  -H "Content-Type: application/json" \
  -d '{"msgType": "TEST_DATA", "content": "integration_test_plain"}'

echo "📡 发送加密消息验证..."
# 模拟加密逻辑
ENC_PAYLOAD=$(node -e '
const crypto = require("crypto");
const secret = "your_app_secret_placeholder";
const key = secret.substring(0, 16);
const iv = secret.substring(0, 16);
const cipher = crypto.createCipheriv("aes-128-cbc", key, iv);
let enc = cipher.update(JSON.stringify({msgType: "APP_TICKET", appTicket: "ticket-integration-success"}), "utf8", "base64");
enc += cipher.final("base64");
console.log(JSON.stringify({encryptMsg: enc}));
')

curl -s -X POST http://localhost:8080/internal/v1/webhook/dispatch \
  -H "X-C-APP_KEY: 3qMYSkA5" \
  -H "X-MSG-ID: msg-enc-002" \
  -H "Content-Type: application/json" \
  -d "$ENC_PAYLOAD"

sleep 3

# 6. 结果校验
echo -e "\n🔍 [Step 4/4] 校验集成测试结果..."
if grep -q "integration_test_plain" demo_full_test.log && grep -q "ticket-integration-success" demo_full_test.log; then
    echo -e "${GREEN}🎉 SUCCESS: 全链路集成验证成功！${NC}"
else
    echo -e "${RED}❌ FAILED: 客户端未接收到预期的测试消息。${NC}"
    echo "--- Demo Log ---"
    cat demo_full_test.log
    exit 1
fi

# 7. 清理
echo -e "\n🧹 正在清理进程..."
kill $GW_PID $DEMO_PID 2>/dev/null || true
rm -f gateway_full_test.log demo_full_test.log
cd ../..

echo "===================================================="
echo -e "${GREEN}✅ 所有测试任务圆满完成！${NC}"
echo "===================================================="
