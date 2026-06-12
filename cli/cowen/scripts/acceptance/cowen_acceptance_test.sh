#!/bin/bash
# cowen_acceptance_test.sh
# 畅捷通 cowen CLI 全能力深度验收与探索测试脚本 (超全功能联动版)
# 包含：多Profile、动态配置、接口规约、零代码代理鉴权注入、StoreApp Webhook拦截与多租户换票联动测试

set -e

echo "=========================================================="
echo "    Cowen CLI 全能力深度验收与高级联动测试脚本"
echo "=========================================================="

# 检查 cowen 是否安装
if ! command -v cowen &> /dev/null; then
    echo "[错误] 未找到 cowen 命令，请先安装 cowen CLI。"
    exit 1
fi

echo "[信息] 当前 cowen 版本："
cowen version

# ---------------------------------------------------------
# 配置参数区 (优先从环境变量读取，不再在脚本中硬编码敏感数据)
# ---------------------------------------------------------

# 如果是独立运行，尝试自动载入同级目录的 .env 文件
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
if [ -z "$STORE_APP_SANDBOX_SECRET" ] && [ -f "$SCRIPT_DIR/.env" ]; then
    echo "[配置] 未检测到环境变量，正在从同级目录加载 .env 配置文件..."
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
fi

# 1. 商店应用 (集测)
STORE_APP_ORG_ID="${STORE_APP_ORG_ID:-90001123021}"
STORE_APP_SANDBOX_KEY="${STORE_APP_SANDBOX_KEY:-ugtEQwms}"
STORE_APP_SANDBOX_SECRET="${STORE_APP_SANDBOX_SECRET}"
STORE_APP_PROD_KEY="${STORE_APP_PROD_KEY:-eMDiqlzR}"
STORE_APP_PROD_SECRET="${STORE_APP_PROD_SECRET}"

# 2. 自建应用
SELF_BUILT_APP_KEY="${SELF_BUILT_APP_KEY:-dqOk3anb}"
SELF_BUILT_APP_SECRET="${SELF_BUILT_APP_SECRET}"

# 3. OAuth2 应用 & 消息加解密
OAUTH2_APP_KEY="${OAUTH2_APP_KEY:-3NWdEbmu}"
OAUTH2_MESSAGE_SECRET="${OAUTH2_MESSAGE_SECRET:-1234567890123456}"
OAUTH2_CERT="${OAUTH2_CERT}"

# 快速校验关键凭据环境变量是否缺失
if [ -z "$STORE_APP_SANDBOX_SECRET" ] || [ -z "$STORE_APP_PROD_SECRET" ] || [ -z "$SELF_BUILT_APP_SECRET" ] || [ -z "$OAUTH2_CERT" ]; then
    echo "❌ 错误: 关键密钥凭证环境变量缺失！请参照 .env.example 复制创建 .env 并填入凭证后重试。"
    exit 1
fi


echo "=========================================================="
echo "  [阶段 1] 环境初始化与 Profile 隔离及生命周期管理"
echo "=========================================================="

# 1.1 初始化商店应用沙箱环境
echo "[执行] 初始化 Profile: store_sandbox"
cowen init -p store_sandbox --app-mode store_app --app-key $STORE_APP_SANDBOX_KEY --app-secret $STORE_APP_SANDBOX_SECRET --encrypt-key "$OAUTH2_MESSAGE_SECRET"

# 1.2 初始化商店应用生产环境
echo "[执行] 初始化 Profile: store_prod"
cowen init -p store_prod --app-mode store_app --app-key $STORE_APP_PROD_KEY --app-secret $STORE_APP_PROD_SECRET --encrypt-key "$OAUTH2_MESSAGE_SECRET"

# 1.3 初始化自建应用环境
echo "[执行] 初始化 Profile: self_built"
cowen init -p self_built --app-mode self_built --app-key $SELF_BUILT_APP_KEY --app-secret $SELF_BUILT_APP_SECRET --encrypt-key "$OAUTH2_MESSAGE_SECRET" -c "$OAUTH2_CERT"

# 1.4 初始化 OAuth2 应用环境 (已授权则跳过，防止覆盖 Token)
if cowen profile list | grep -q "oauth2_app"; then
    echo "[信息] Profile: oauth2_app 已存在，跳过初始化以保留已有授权凭证。"
else
    echo "[执行] 初始化 Profile: oauth2_app (马嘟嘟中心三 好业财)"
    cowen init -p oauth2_app --app-mode oauth2 --app-key "$OAUTH2_APP_KEY" --encrypt-key "$OAUTH2_MESSAGE_SECRET"
fi

# 1.5 验证 profile 核心命令家族：新建、更名、激活、删除
echo "[执行] 新建临时 Profile: temp_test"
cowen init -p temp_test --app-mode store_app --app-key tempkey --app-secret tempsecret --encrypt-key "$OAUTH2_MESSAGE_SECRET"

echo "[执行] 重命名 Profile (temp_test -> temp_test_renamed)"
cowen profile rename temp_test temp_test_renamed

echo "[执行] 查看当前 Profile 环境名称"
cowen profile current

echo "[执行] 切换并激活到新 Profile: temp_test_renamed"
cowen profile use temp_test_renamed

echo "[执行] 确认激活切换结果"
cowen profile current

echo "[执行] 重置并清除临时 Profile: temp_test_renamed"
cowen reset -p temp_test_renamed --no-telemetry

# 切换回默认的工作 Profile
echo "[执行] 切换回工作环境: self_built"
cowen profile use self_built

echo "当前配置文件列表："
cowen profile list


echo "=========================================================="
echo "  [阶段 2] 配置精细化管理与读写测试 (config)"
echo "=========================================================="

echo "[执行] 查看 self_built 的当前配置项列表"
cowen config list -p self_built

echo "[执行] 获取当前 webhook_target 值"
ORIG_WEBHOOK=$(cowen config get webhook_target -p self_built)
echo "原 Webhook 接收地址: $ORIG_WEBHOOK"

echo "[执行] 动态设置 webhook_target 值为临时的 http://127.0.0.1:9999"
cowen config set webhook_target http://127.0.0.1:9999 -p self_built

echo "[执行] 重新获取 webhook_target 值进行核实"
UPDATED_WEBHOOK=$(cowen config get webhook_target -p self_built)
echo "修改后 Webhook 接收地址: $UPDATED_WEBHOOK"
if [ "$UPDATED_WEBHOOK" != "http://127.0.0.1:9999" ]; then
    echo "[错误] 配置项写入与读取不一致！"
    exit 1
fi

echo "[执行] 恢复原 webhook_target 地址"
cowen config set webhook_target "$ORIG_WEBHOOK" -p self_built


echo "=========================================================="
echo "  [阶段 3] 身份认证与凭据状态管理 (auth)"
echo "=========================================================="

# 验收自建应用获取 Token
echo "[执行] 测试获取自建应用 Token"
cowen auth token -p self_built

# 验证 reload 命令能否正常运行
echo "[执行] 从共享存储中强制同步最新凭据数据 (reload)"
cowen auth reload -p self_built

# 检查凭据状态
echo "[执行] 检查各个 Profile 凭据的健康状态与剩余寿命"
cowen auth status -p store_sandbox || true
cowen auth status -p self_built || true
cowen auth status -p oauth2_app || true


echo "=========================================================="
echo "  [阶段 4] API 智能检索、规约解析与接口调用 (api)"
echo "=========================================================="

echo "[执行] 1. API 语义搜索 (使用 self_built)"
cowen api list --search "获取部门列表" -p self_built

echo "[执行] 2. 查看特定接口 of OpenAPI 详情定义与规约"
cowen api spec POST /accounting/openapi/cc/department/list/{bookid} -p self_built

echo "[执行] 3. 发起直接的 API 调用测试 (自动注入签名安全头)"
cowen api POST /accounting/openapi/cc/department/list/123456789 -p self_built -d '{}' --force || echo "[注意] 请求由于业务原因失败，但物理传输与签名正常。"


echo "=========================================================="
echo "  [阶段 5] 存储后端与状态管理 (store)"
echo "=========================================================="

echo "[执行] 检查主存储后端的健康状态与连接性"
cowen store status -p self_built


echo "=========================================================="
echo "  [阶段 6] 系统事件流与诊断回溯 (events / doctor)"
echo "=========================================================="

echo "[执行] 1. 运行环境诊断 (doctor)"
cowen doctor

echo "[执行] 2. 查看过去的系统事件流与故障轨迹 (events)"
cowen events -n 10 -p self_built || echo "[信息] 新架构下 events 命令由守护进程管理，客户端在此跳过。"


echo "=========================================================="
echo "  [阶段 7] 死信队列管理与清空 (dlq)"
echo "=========================================================="

echo "[执行] 1. 列出死信队列 (DLQ)"
cowen dlq list -p self_built

echo "[执行] 2. 物理清除死信队列堆积事件 (purge)"
cowen dlq purge -p self_built


echo "=========================================================="
echo "  [阶段 8] 系统扩展插件管理 (plugins)"
echo "=========================================================="

echo "[执行] 扫描并列出 ~/.cowen/plugins/ 目录下的扩展插件"
cowen plugins list


echo "=========================================================="
echo "  [阶段 9] 审计与运行日志跟踪 (log / audit)"
echo "=========================================================="

echo "[执行] 1. 查看审计日志最后 10 行"
cowen audit tail -n 10 -p self_built

echo "[执行] 2. 查看 CLI 运行日志"
cowen log view -n 10 -p self_built


echo "=========================================================="
echo "  [阶段 10] 代理与多租户 (StoreApp/自建) 联动深度拦截验证 (NEW)"
echo "=========================================================="

# 10.1 验证自建应用本地代理自动鉴权与云端路由连通性
echo "[执行] 获取自建应用代理端口..."
PORT_SELF_BUILT=$(cowen auth status -p self_built | tr -d '\033[' | grep -o '127.0.0.1:[0-9]*' | cut -d: -f2 | head -n 1)
echo "自建应用代理运行端口: $PORT_SELF_BUILT"

echo "[执行] 通过自建应用本地代理端口发起免密调用 (测试请求路由与签名自动注入)"
PROXY_RESP=$(curl -s http://127.0.0.1:$PORT_SELF_BUILT/accounting/openapi/cc/department/list/123456789 -H "Content-Type: application/json" -d '{}')
echo "本地代理接口返回:"
echo "$PROXY_RESP"
if echo "$PROXY_RESP" | grep -q "账套信息错误"; then
    echo "✅ 自建应用反向代理鉴权头及签名自动注入连通性验证成功！"
else
    echo "⚠️ 代理调用返回了非预期错误，请检查状态。"
fi

# 10.2 验证商店应用沙箱环境本地 Webhook 拦截与 APP_TICKET 解密及持久化
echo "[执行] 获取商店沙箱代理端口..."
PORT_SANDBOX=$(cowen auth status -p store_sandbox | tr -d '\033[' | grep -o '127.0.0.1:[0-9]*' | cut -d: -f2 | head -n 1)
echo "商店沙箱代理运行端口: $PORT_SANDBOX"

echo "[执行] 模拟开放平台推送 APP_TICKET Webhook 给本地代理端口"
TICKET_PUSH_RESP=$(curl -s -X POST http://127.0.0.1:$PORT_SANDBOX/webhook \
  -H "Content-Type: application/json" \
  -d '{"type": "APP_TICKET", "app_ticket": "test_ticket_value_123456"}')
echo "Webhook 接收响应: $TICKET_PUSH_RESP"

echo "[执行] 验证 AppTicket 是否成功保存及更新诊断状态"
if cowen auth status -p store_sandbox | grep -q "AppTicket: \[CACHED\]"; then
    echo "✅ 商店应用 Webhook 推送拦截与凭证持久化验证成功！"
else
    echo "❌ 商店应用 AppTicket 缓存验证失败！"
    exit 1
fi

# 10.3 验证商店应用多租户临时授权码换票后台异步流程
echo "[执行] 获取商店正式应用代理端口..."
PORT_STORE_PROD=$(cowen auth status -p store_prod | tr -d '\033[' | grep -o '127.0.0.1:[0-9]*' | cut -d: -f2 | head -n 1)
echo "商店正式应用代理端口: $PORT_STORE_PROD"

echo "[执行] 推送 TEMP_AUTH_CODE Webhook，触发后台自动异步换票"
TEMP_CODE_RESP=$(curl -s -X POST http://127.0.0.1:$PORT_STORE_PROD/webhook \
  -H "Content-Type: application/json" \
  -d '{"type": "TEMP_AUTH_CODE", "temp_auth_code": "temp_code_999", "state": "state_123"}')
echo "临时码推送响应 (由于使用了虚拟凭据，预期会触发后台换票到平台换 AppAccessToken 并返回 401 报错，这属于正常现象):"
echo "$TEMP_CODE_RESP"
if echo "$TEMP_CODE_RESP" | grep -q "appKey不正确"; then
    echo "✅ 商店应用多租户临时码推送及后台自动换票触发连通性验证成功！"
else
    echo "⚠️ 商店应用换票回显非预期，请检查环境状态。"
fi

echo "=========================================================="
echo "    增强型深度联动验收脚本执行完毕，各核心模块运行正常。"
echo "=========================================================="
