#!/bin/bash

# 稳定性测试脚本 (2小时)
# 目标：验证 WebSocket 长连接稳定性及消息到达率

# 1. 环境准备
ENV_FILE="sdk/nodejs-demo/.env"
if [ ! -f "$ENV_FILE" ]; then
    echo "Error: .env file not found"
    exit 1
fi

APP_KEY=$(grep APP_KEY $ENV_FILE | cut -d '=' -f2)
ENCRYPT_KEY=$(grep ENCRYPT_KEY $ENV_FILE | cut -d '=' -f2)
GATEWAY_URL=$(grep GATEWAY_URL $ENV_FILE | cut -d '=' -f2)
LOG_FILE="sdk/nodejs-demo/stability_demo.log"
REPORT_FILE="stability_report.md"

echo "===================================================="
echo "🛡️ 开始稳定性验证 (预计时长: 120 分钟)"
echo "📍 目标网关: $GATEWAY_URL"
echo "📍 记录日志: $LOG_FILE"
echo "===================================================="

# 2. 启动 Demo 客户端
cd sdk/nodejs && npm run build > /dev/null && cd ../..
cd sdk/nodejs-demo
nohup node main.js > stability_demo.log 2>&1 &
DEMO_PID=$!
echo "✅ Demo 已在后台启动 (PID: $DEMO_PID)"
cd ../..

# 3. 循环发送 Webhook (120次，每分钟一次)
START_TIME=$(date +%s)
SUCCESS_SENT=0

for i in {1..120}
do
    # 生成加密 Payload
    PAYLOAD=$(node -e "
const crypto = require('crypto');
const secret = '$ENCRYPT_KEY';
const cipher = crypto.createCipheriv('aes-128-ecb', Buffer.from(secret), null);
let enc = cipher.update(JSON.stringify({msgType: 'APP_TICKET', appTicket: 'stab-test-' + Date.now()}), 'utf8', 'base64');
enc += cipher.final('base64');
console.log(JSON.stringify({encryptMsg: enc}));
")

    # 发送请求
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$GATEWAY_URL/internal/v1/webhook/dispatch" \
      -H "X-C-APP_KEY: $APP_KEY" \
      -H "X-MSG-ID: stab-$i-$(date +%s)" \
      -H "Content-Type: application/json" \
      -d "$PAYLOAD")

    if [ "$HTTP_CODE" == "200" ]; then
        ((SUCCESS_SENT++))
    fi

    echo "[$i/120] $(date '+%H:%M:%S') Webhook 已发送 (Status: $HTTP_CODE). 已成功: $SUCCESS_SENT"
    sleep 60
done

END_TIME=$(date +%s)
DURATION=$(( (END_TIME - START_TIME) / 60 ))

# 4. 分析日志并生成报告
echo "===================================================="
echo "📊 测试结束，正在生成分析报告..."
echo "===================================================="

# 统计数据
TOTAL_RECEIVED=$(grep -c "收到应用票据" $LOG_FILE || echo 0)
RECONNECT_COUNT=$(grep -c "Attempting to connect" $LOG_FILE || echo 0)
CONN_SUCCESS=$(grep -c "WebSocket connected" $LOG_FILE || echo 0)
ERROR_COUNT=$(grep -c "Error" $LOG_FILE || echo 0)

cat <<EOF > $REPORT_FILE
# 稳定性验证报告 (Open Streaming Connector)

## 1. 测试概览
- **测试时间**: $(date -r $START_TIME) ~ $(date -r $END_TIME)
- **持续时长**: $DURATION 分钟
- **目标地址**: $GATEWAY_URL
- **AppKey**: $APP_KEY

## 2. 消息到达率分析
- **模拟发送总数**: 120
- **网关接收成功 (200 OK)**: $SUCCESS_SENT
- **客户端实际解密处理数**: $TOTAL_RECEIVED
- **最终到达率**: $(echo "scale=2; $TOTAL_RECEIVED / 120 * 100" | bc)%

## 3. 连接稳定性分析
- **连接尝试次数**: $RECONNECT_COUNT
- **连接成功次数**: $CONN_SUCCESS
- **重连率**: $(echo "scale=2; ($RECONNECT_COUNT - 1) / $DURATION" | bc) 次/分钟
- **异常日志数**: $ERROR_COUNT

## 4. 结论
$( [ "$TOTAL_RECEIVED" -ge 115 ] && echo "✅ **通过**：消息到达率符合预期 (>=95%)。" || echo "❌ **未通过**：消息丢失严重，请检查网络或网关负载能力。")
$( [ "$RECONNECT_COUNT" -le 5 ] && echo "✅ **连接稳定**：测试期间重连次数较少。" || echo "⚠️ **连接抖动**：检测到频繁重连，请关注负载均衡或网络质量。")

EOF

kill $DEMO_PID
echo "✅ 报告已生成: $REPORT_FILE"
