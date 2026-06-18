#!/usr/bin/env bash
# case_85_sealed_storage.sh
# Tests the encrypted/sealed storage engine (MonolithicSealStore)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_85_sealed"
setup_workspace "case_85_$PROFILE"
cd "$COWEN_HOME"

# 1. 编写包含 local 存储的 app.yaml 配置文件
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: local
log:
  level: debug
EOF

# 2. 在工作区根目录下 touch .seal 文件，激活 MonolithicSealStore 密封加密存储
touch "$COWEN_HOME/.seal"

echo "🔐 1. 初始化加密储存环境..."
"$COWEN_BIN" init --profile "$PROFILE" --app-key "dummykey" --app-secret "dummysecret" --app-mode self-built --certificate "dummy_cert" --encrypt-key "supersecret"

# 检查加密的 vault 目录是否已生成
if [ ! -d "$COWEN_HOME/vault" ]; then
    fail_suite "vault directory was not created under sealed mode"
fi

# 确认 .seal 文件依然存在
if [ ! -f "$COWEN_HOME/.seal" ]; then
    fail_suite ".seal marker file missing"
fi
assert_pass "Initialized vault in sealed storage mode"

echo "🔐 2. 测试加密读写配置项..."
# 写入一条配置
"$COWEN_BIN" --profile "$PROFILE" config set "webhook_target" "http://localhost:9999"

# 读取校验
VAL=$("$COWEN_BIN" --profile "$PROFILE" config get "webhook_target")
if [ "$VAL" != "http://localhost:9999" ]; then
    fail_suite "Config get mismatch. Expected 'http://localhost:9999', got '$VAL'"
fi
assert_pass "Write and read config inside sealed store successfully"

echo "🔐 3. 校验敏感密码/密钥存储..."
# 直接从敏感存储中解密读取校验
SEC_VAL=$("$COWEN_BIN" --profile "$PROFILE" config get "encrypt_key")
if [ "$SEC_VAL" != "supersecret" ]; then
    fail_suite "Secret read mismatch. Got '$SEC_VAL'"
fi
assert_pass "Write and read secrets in sealed store successfully"

# 4. 列出所有配置项
CONFIGS=$("$COWEN_BIN" --profile "$PROFILE" config list)
if ! echo "$CONFIGS" | grep -q "webhook_target"; then
    fail_suite "Config key 'webhook_target' missing from list output"
fi
assert_pass "List configs inside sealed store successfully"

echo "✅ Sealed Local Storage tests Passed!"
