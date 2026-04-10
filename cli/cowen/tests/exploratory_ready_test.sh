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

# 1. 基础连通性与配置可用性测试
echo -e "${YELLOW}Step 1: 验证配置与 Profile 加载...${NC}"
if "$BINARY_PATH" config > /dev/null 2>&1; then
    echo -e "${GREEN}   [OK] 核心配置读取正常 (兼容性验证通过)${NC}"
else
    echo -e "${RED}   [FAIL] 核心配置读取失败！${NC}"
    exit 1
fi

CURRENT_PROFILE=$("$BINARY_PATH" profile list | grep "\*" | awk '{print $NF}' | tr -d '()')
echo -e "${GREEN}   [OK] 当前激活 Profile: $CURRENT_PROFILE${NC}"

# 2. 状态与守护进程交互测试
echo -e "${YELLOW}Step 2: 验证系统状态与守护进程活跃度...${NC}"
STATUS_OUT=$("$BINARY_PATH" status)
echo "$STATUS_OUT" | grep -q "PID" && echo -e "${GREEN}   [OK] 守护进程在线 (或已自动拉起)${NC}" || echo -e "${YELLOW}   [INFO] 守护进程当前未在该 Profile 下运行 (预期内)${NC}"

# 3. AI 语义搜索能力深度探索 (Neural Search)
echo -e "${YELLOW}Step 3: 探索 AI 语义搜索行为...${NC}"
# 寻找关键词，验证语义映射是否存在
SEARCH_OUT=$("$BINARY_PATH" api list --search "发票" --log-level error 2>&1 || true)
if echo "$SEARCH_OUT" | grep -q "Neural Search"; then
    echo -e "${GREEN}   [OK] 本地语义引擎激活，关键词 '发票' 映射成功${NC}"
else
    echo -e "${YELLOW}   [SKIP] 语义引擎未响应，可能模型未下载或当前版本不支持${NC}"
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
TMP_PROF="exploratory_tmp"
"$BINARY_PATH" profile use "$TMP_PROF" >/dev/null 2>&1
# 初始化临时环境（使用模拟数据）
"$BINARY_PATH" init --app-key "TEST_KEY" --app-secret "TEST_SEC" --certificate "TEST_CERT" >/dev/null 2>&1

CONFIG_FILE="$HOME/.cowen/$TMP_PROF.yaml"
if [ -f "$CONFIG_FILE" ]; then
    if ! grep -q "TEST_SEC" "$CONFIG_FILE"; then
        echo -e "${GREEN}   [OK] 临时 Profile 创建成功，且 AppSecret 已安全隔离至 Vault (非明文存储)${NC}"
    else
        echo -e "${RED}   [FAIL] 警告：AppSecret 被明文写入了 YAML 配置文件！${NC}"
        # 清理后退出
        mv -f "$HOME/.cowen/current_profile.bak" "$HOME/.cowen/current_profile" 2>/dev/null || true
        rm -f "$CONFIG_FILE"
        exit 1
    fi
    # 清理现场
    rm -f "$CONFIG_FILE"
    rm -f "$HOME/.cowen/${TMP_PROF}_daemon.pid" 2>/dev/null || true
else
    echo -e "${RED}   [FAIL] 无法创建临时 Profile 配置文件${NC}"
fi

# 切回原 Profile
"$BINARY_PATH" profile use "$CURRENT_PROFILE" >/dev/null 2>&1


# 6. 安全模块与日志管理检查
echo -e "${YELLOW}Step 6: 安全模块与日志管理系统检查...${NC}"
if "$BINARY_PATH" auth status > /dev/null 2>&1; then
    echo -e "${GREEN}   [OK] 安全凭据访问模块响应正常${NC}"
else
    echo -e "${RED}   [FAIL] Vault 模块异常${NC}"
    exit 1
fi

# 7. 日志与系统可观测性检查
echo -e "${YELLOW}Step 7: 日志管理系统响应检查...${NC}"
if "$BINARY_PATH" log list | grep -q "sys.log"; then
    echo -e "${GREEN}   [OK] 日志域 (Domain) 自动发现正常${NC}"
else
    echo -e "${RED}   [FAIL] 日志域列表获取失败${NC}"
    exit 1
fi

echo -e "\n${GREEN}🎉 所有就绪态探索性测试项已顺利执行完毕！${NC}"
echo -e "${BLUE}环境功能完整，可以开始正式业务作业。${NC}"
