#!/bin/bash
# cowen CLI Exploratory Ready Test Script
# 用于在环境就绪（编译完成、基础配置存在）后执行的深度功能探索性测试

set -e

# 设置 ANSI 颜色
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BINARY_PATH=$1

if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
    echo -e "${RED}❌ 错误: 未指定二进制产物路径或文件不存在。${NC}"
    echo "用法: $0 <path_to_binary>"
    exit 1
fi

echo -e "${BLUE}🚀 启动就绪态功能探索性测试 (Target: $BINARY_PATH)...${NC}"

# 获取并保存初始 Profile，设置退出后的清理陷阱
INITIAL_PROFILE=$("$BINARY_PATH" profile current | tail -n 1 | awk '{print $NF}')
TMP_PROF="exploratory_tmp"

cleanup() {
    echo -e "\n${YELLOW}🧹 正在清理临时测试环境 ($TMP_PROF)...${NC}"
    # 1. 尝试优雅停止守护进程
    "$BINARY_PATH" daemon stop --profile "$TMP_PROF" >/dev/null 2>&1 || true
    # 2. 强力清理残留进程 (针对测试 Profile)
    pkill -f "cowen.*--profile $TMP_PROF" >/dev/null 2>&1 || true
    # 3. 调用 reset 彻底清除 Vault 指纹、配置和缓存
    "$BINARY_PATH" reset --profile "$TMP_PROF" >/dev/null 2>&1 || true
    # 4. 还原到初始 Profile
    "$BINARY_PATH" profile use "$INITIAL_PROFILE" >/dev/null 2>&1 || true
    # 5. 彻底物理删除遗留文件
    rm -f "$HOME/.cowen/$TMP_PROF.yaml" "$HOME/.cowen/${TMP_PROF}_daemon.pid" 2>/dev/null || true
    echo -e "${GREEN}✅ 清理完成，已还原至 Profile: $INITIAL_PROFILE${NC}"
}

trap cleanup EXIT

# 1. 基础连通性与配置可用性测试
echo -e "${YELLOW}Step 1: 验证配置与 Profile 加载...${NC}"
if "$BINARY_PATH" config > /dev/null 2>&1; then
    echo -e "${GREEN}   [OK] 核心配置读取正常 (兼容性验证通过)${NC}"
else
    echo -e "${RED}   [FAIL] 核心配置读取失败！${NC}"
    exit 1
fi

PROFILE_LIST=$("$BINARY_PATH" profile list)
if echo "$PROFILE_LIST" | grep -q "_openapi"; then
    echo -e "${RED}   [FAIL] 警告：Profile 列表中发现了非环境文件 (如 _openapi 规约缓存)${NC}"
    exit 1
else
    echo -e "${GREEN}   [OK] Profile 列表清理通过 (已排除元数据文件)${NC}"
fi

CURRENT_PROFILE=$("$BINARY_PATH" profile current | awk '{print $NF}')
echo -e "${GREEN}   [OK] 当前激活 Profile: $CURRENT_PROFILE${NC}"

# 2. 状态诊断与全量状态矩阵探测
echo -e "${YELLOW}Step 2: 验证全量状态诊断 (status --all)...${NC}"
STATUS_ALL=$("$BINARY_PATH" status --all)
PROF_COUNT=$(echo "$STATUS_ALL" | grep -c "Profile:" || true)
if [ "$PROF_COUNT" -gt 0 ]; then
    echo -e "${GREEN}   [OK] status --all 响应正常，检测到 $PROF_COUNT 个活跃 Profile 状态矩阵${NC}"
else
    echo -e "${RED}   [FAIL] status --all 未能正确获取 Profile 状态列表${NC}"
    exit 1
fi

# 4. 配置响应格式与掩码 (Output Format & Masking)
echo -e "${YELLOW}Step 4: 验证输出格式与敏感信息脱敏...${NC}"
JSON_OUT=$("$BINARY_PATH" config -o json 2>&1 || true)
if echo "$JSON_OUT" | grep -q "app_key"; then
    if ! echo "$JSON_OUT" | grep -qE "app_secret|certificate|encrypt_key"; then
        echo -e "${GREEN}   [OK] JSON 格式输出正常，且敏感字段 (Vault keys) 已自动剥离${NC}"
    else
        echo -e "${RED}   [FAIL] 警告：发现敏感字段通过 JSON 明文泄露！${NC}"
        exit 1
    fi
else
    echo -e "${RED}   [FAIL] JSON 格式解析异常${NC}"
    exit 1
fi

# 5. Profile 隔离性与持久化探索
echo -e "${YELLOW}Step 5: 探索 Profile 隔离与 Vault 机制...${NC}"
"$BINARY_PATH" profile use "$TMP_PROF" >/dev/null 2>&1
# 初始化临时环境（使用模拟数据）
"$BINARY_PATH" init --profile "$TMP_PROF" --app-mode self-built --app-key "TEST_KEY" --app-secret "TEST_SEC" --certificate "TEST_CERT" --encrypt-key "TEST_ENC" >/dev/null 2>&1

CONFIG_FILE="$HOME/.cowen/$TMP_PROF.yaml"
if [ -f "$CONFIG_FILE" ]; then
    if ! grep -q "TEST_SEC" "$CONFIG_FILE"; then
        echo -e "${GREEN}   [OK] 临时 Profile 创建成功，且 AppSecret 已安全隔离至 Vault (非明文存储)${NC}"
    else
        echo -e "${RED}   [FAIL] 警告：AppSecret 被明文写入了 YAML 配置文件！${NC}"
        exit 1
    fi
else
    echo -e "${RED}   [FAIL] 无法创建临时 Profile 配置文件${NC}"
    exit 1
fi

# 预置一个极简的本地 Spec 缓存，防止 Proxy 因为获取规约失败而报 500
echo '{"openapi":"3.0.0","paths":{"/v1/user":{"get":{"summary": "查询用户信息", "description": "获取当前登录用户的详细信息", "responses":{"200":{"description":"OK"}}}}}}' > "$HOME/.cowen/${TMP_PROF}_openapi.yaml"

# 3. AI 语义搜索能力深度探索 (Neural Search)
echo -e "${YELLOW}Step 3: 探索 AI 语义搜索行为...${NC}"
# 寻找关键词，验证语义映射是否存在
# 注意：第一次搜索会触发索引构建
SEARCH_OUT=$("$BINARY_PATH" api list --search "用户" --log-level error 2>&1 || true)
if echo "$SEARCH_OUT" | grep -q "Neural Search"; then
    echo -e "${GREEN}   [OK] 本地语义引擎激活，关键词 '用户' 映射成功${NC}"
else
    # 如果还是失败，输出错误日志便于调试
    echo -e "${RED}   [FAIL] 语义搜索失败！输出: $SEARCH_OUT${NC}"
    exit 1
fi


# 6. 安全模块与日志管理检查
echo -e "${YELLOW}Step 6: 安全模块与日志管理系统检查...${NC}"
if "$BINARY_PATH" auth status > /dev/null 2>&1; then
    echo -e "${GREEN}   [OK] 安全凭据访问模块响应正常${NC}"
else
    echo -e "${RED}   [FAIL] Vault 模块异常${NC}"
    exit 1
fi

# 7. 日志与系统可观测性检查 (隔离性验证)
echo -e "${YELLOW}Step 7: 日志管理系统响应检查 (隔离性验证)...${NC}"
LOG_DIR="$HOME/.cowen/logs"
touch "$LOG_DIR/other_profile_dummy.log"

LOG_LIST=$("$BINARY_PATH" log list)
if echo "$LOG_LIST" | grep -q "other_profile_dummy.log"; then
    echo -e "${RED}   [FAIL] 警告：日志列表泄漏！发现了非当前 Profile 的日志文件${NC}"
    rm -f "$LOG_DIR/other_profile_dummy.log"
    exit 1
else
    echo -e "${GREEN}   [OK] 日志隔离正常 (仅显示全局日志及当前 Profile 日志)${NC}"
    rm -f "$LOG_DIR/other_profile_dummy.log"
fi

if echo "$LOG_LIST" | grep -q "sys.log"; then
    echo -e "${GREEN}   [OK] 日志域 (Domain) 自动发现正常${NC}"
else
    echo -e "${RED}   [FAIL] 日志域列表获取失败${NC}"
    exit 1
fi

# 8. 验证本地代理服务器 (Local Proxy) 功能...
echo -e "${YELLOW}Step 8: 验证本地代理 (Local Proxy) 功能...${NC}"
# 开启后台代理
"$BINARY_PATH" daemon stop --profile "$TMP_PROF" > /dev/null 2>&1 || true
"$BINARY_PATH" daemon start --profile "$TMP_PROF" --enable-proxy --proxy-port 9091 > /dev/null 2>&1

# 循环等待代理端口就绪 (最多等待 5s)
MAX_WAIT=10
COUNT=0
echo -n "   [WAIT] 等待代理服务器启动..."
while ! nc -z 127.0.0.1 9091 >/dev/null 2>&1; do
    sleep 0.5
    echo -n "."
    COUNT=$((COUNT + 1))
    if [ $COUNT -ge $MAX_WAIT ]; then
        echo -e "\n${RED}   [FAIL] 代理服务器启动超时 (9091)${NC}"
        # 记录下最后的系统日志查看原因
        tail -n 10 "$HOME/.cowen/logs/sys.log"
        exit 1
    fi
done
echo -e " [READY]"

# 验证 1: 访问非法路径 (应被 403 拦截)
PROXY_403=$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:9091/invalid/path)
if [ "$PROXY_403" == "403" ]; then
    echo -e "${GREEN}   [OK] Proxy 拦截逻辑正常 (403 Forbidden for non-whitelist)${NC}"
else
    echo -e "${RED}   [FAIL] Proxy 未能正确拦截非法路径 (Code: $PROXY_403)${NC}"
    exit 1
fi

# 验证 2: 访问合法路径但无凭据 (应尝试注入并返回 401/200 取决于环境，此处验证是否透出了 Proxy 的审计日志)
curl -s http://127.0.0.1:9091/v1/user > /dev/null 2>&1 || true
if grep -q "Proxy Rejected" "$HOME/.cowen/logs/audit.log"; then
    echo -e "${GREEN}   [OK] Proxy 审计日志记录正常${NC}"
else
    # 如果是因为没有 Token 导致的 401，也会有日志
    if grep -q "Failed to get access token for proxy" "$HOME/.cowen/logs/audit.log"; then
         echo -e "${GREEN}   [OK] Proxy 审计日志记录正常 (Token 缺失链路已触达)${NC}"
    else
         echo -e "${YELLOW}   [WARN] Proxy 审计日志未找到匹配项，请检查日志域配置${NC}"
    fi
fi

# 停止后台代理
"$BINARY_PATH" daemon stop --profile "$TMP_PROF" > /dev/null 2>&1 || true

# 9. 探索 Reset 彻底性 (DLQ & 滚动日志清理)
echo -e "${YELLOW}Step 9: 验证 Reset 清理彻底性 (DLQ & 滚动日志)...${NC}"
# 模拟产生 DLQ 目录和滚动日志
DLQ_PROF_DIR="$HOME/.cowen/dlq/$TMP_PROF"
mkdir -p "$DLQ_PROF_DIR"
touch "$DLQ_PROF_DIR/failed_event.json"
touch "$LOG_DIR/${TMP_PROF}_sys.log.1"

# 执行 Reset
"$BINARY_PATH" reset --profile "$TMP_PROF" >/dev/null 2>&1

if [ -d "$DLQ_PROF_DIR" ]; then
    echo -e "${RED}   [FAIL] 警告：Reset 未能删除 DLQ 专用目录${NC}"
    exit 1
fi

if [ -f "$LOG_DIR/${TMP_PROF}_sys.log.1" ]; then
    echo -e "${RED}   [FAIL] 警告：Reset 未能清理滚动生成的日志文件${NC}"
    exit 1
fi
echo -e "${GREEN}   [OK] Reset 逻辑通过：已彻底物理粉碎临时 Profile 的所有痕迹${NC}"

# 10. 安全日志脱敏专项验证 (Log Masking)
echo -e "${YELLOW}Step 10: 验证安全日志脱敏 (Body & URL)...${NC}"
TEST_SECRET="SUPER_SECRET_TOKEN_999"
# 触发一个带敏感参数的 API 调用 (即使失败，日志也应产生)
"$BINARY_PATH" api GET "/v1/test?accessToken=$TEST_SECRET" --data "{\"password\": \"$TEST_SECRET\"}" --profile "$TMP_PROF" >/dev/null 2>&1 || true

SYS_LOG="$HOME/.cowen/logs/sys.log"
AUDIT_LOG="$HOME/.cowen/logs/audit.log"

if grep -q "$TEST_SECRET" "$SYS_LOG" || grep -q "$TEST_SECRET" "$AUDIT_LOG"; then
    echo -e "${RED}   [FAIL] 警告：敏感信息在日志中明文泄漏！${NC}"
    exit 1
else
    echo -e "${GREEN}   [OK] 日志脱敏验证通过 (URL 与 Body 均已成功掩码)${NC}"
fi

# 11. 命令行补全能力检查 (Shell Completion)
echo -e "${YELLOW}Step 11: 验证本地命令行补全脚本生成...${NC}"
COMP_ZSH=$("$BINARY_PATH" completion zsh)
if echo "$COMP_ZSH" | grep -q "compdef cowen"; then
    echo -e "${GREEN}   [OK] ZSH 补全脚本生成正常${NC}"
else
    echo -e "${RED}   [FAIL] 补全脚本生成异常${NC}"
    exit 1
fi

echo -e "\n${GREEN}🎉 所有探索性测试项 (Step 1-11) 已顺利执行完毕！${NC}"
echo -e "${BLUE}环境功能完整，可以开始正式业务作业。${NC}"

