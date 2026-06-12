#!/bin/bash
# run_all_acceptance.sh
# 畅捷通 cowen CLI 一键式端到端全自动验收主控脚本
# 支持自动检查环境、唤起 Chrome CDP 执行 OAuth2 模拟授权、并执行 10 阶段的深度联动功能校验。

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# 载入并导出本地 .env 环境变量
if [ -f "$SCRIPT_DIR/.env" ]; then
    echo "[配置] 检测到本地 .env 文件，正在导入环境变量..."
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
else
    echo "❌ 错误: 未能找到 .env 文件！请参照 .env.example 复制创建 .env 并填入凭证。"
    exit 1
fi

echo "=========================================================="
echo "    🚀 Cowen CLI 一键端到端全自动验收主总控脚本 🚀"
echo "=========================================================="

# 1. 检查基础指令依赖
echo "[步骤 1] 依赖环境与命令校验..."
if ! command -v cowen &> /dev/null; then
    echo "❌ 错误: 未检测到 cowen CLI 命令，请确保其已加入环境变量 PATH 中。"
    exit 1
fi
if ! command -v node &> /dev/null; then
    echo "❌ 错误: 本地需要 Node.js 环境以执行 CDP 自动化页面交互测试。"
    exit 1
fi
echo "✅ 基础 CLI 依赖校验通过。"

# 2. 检查 Chrome 9222 调试端口是否就绪
echo "[步骤 2] 检查 Google Chrome 调试端口 (9222) 是否在线..."
if ! lsof -i :9222 &> /dev/null; then
    echo "⚠️  警告: 未检测到正在运行的 9222 调试端口 Chrome。"
    echo "    请执行以下命令来启动具备调试状态的 Chrome (使用您当前已登录的 Profile 目录副本):"
    echo "    /Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome \\"
    echo "      --remote-debugging-port=9222 \\"
    echo "      --user-data-dir=\"$SCRIPT_DIR/chrome_debug_profile\""
    echo ""
    echo "❌ 无法继续执行自动化 OAuth2 换票。脚本已退出。"
    exit 1
fi
echo "✅ Chrome 9222 调试通道建立成功！"

# 3. 运行 CDP 自动模拟点击进行 OAuth2 换票 (马嘟嘟中心三 好业财)
echo "[步骤 3] 执行 Google Chrome CDP 自动模拟授权流..."
if ! node "$SCRIPT_DIR/cdp_auth_fast.js"; then
    echo "❌ 错误: OAuth2 自动模拟授权失败！请检查调试版 Chrome 的登录会话是否过期。"
    exit 1
fi
echo "✅ OAuth2 自动模拟换票流程圆满成功！"

# 4. 运行全能力与代理联动验收脚本 (10 阶段)
echo "[步骤 4] 启动 10 阶段 CLI 功能及反向代理联动深度验收测试..."
if ! bash "$SCRIPT_DIR/cowen_acceptance_test.sh"; then
    echo "❌ 错误: 核心功能校验不通过！请查看报错信息并调试对应模块。"
    exit 1
fi

echo "=========================================================="
echo "  🎉 恭喜！一键全自动验收执行完毕，所有能力 100% 校验成功！ 🎉"
echo "=========================================================="
echo "已更新本地脱敏验收报告，路径如下："
echo "👉 $SCRIPT_DIR/cowen_acceptance_report.md"
echo "=========================================================="
