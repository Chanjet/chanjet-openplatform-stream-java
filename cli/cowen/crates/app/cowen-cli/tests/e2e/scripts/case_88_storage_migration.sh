#!/usr/bin/env bash
# case_88_storage_migration.sh
# Tests V2 to V3 storage format migrations (monolithic JSON -> split files, and tok_v2 -> tokens)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_88_mig"
setup_workspace "case_88_$PROFILE"
cd "$COWEN_HOME"

# 覆盖默认 app.yaml 以激活 local 存储引擎以便触发迁移
cat > "app.yaml" <<EOF
storage:
  store: local
log:
  level: debug
EOF


echo "📦 1. 物理构筑 V2 历史存储环境..."
# 创建 vault 目录
mkdir -p "vault/$PROFILE"

# 1.1 创建 V2 monolithic 格式的文件 (不加密状态，传 fingerprint=None 进行测试)
# V2 结构为: prefix -> id -> item_json
# 我们可以写入 config 数据
cat > "vault/$PROFILE.json" <<EOF
{
  "config": {
    "key1": {
      "profile": "case_88_mig",
      "key": "key1",
      "value": "val1",
      "version": 1,
      "updated_at": 123456
    }
  }
}
EOF

# 1.2 创建旧的 tok_v2 目录
mkdir -p "vault/$PROFILE/tok_v2"
echo "dummy_token" > "vault/$PROFILE/tok_v2/access_token"

# 1.3 创建旧的 dlq 结构 (dlq/topic/msg_file)
mkdir -p "vault/$PROFILE/dlq/test_topic"
echo "dlq_msg" > "vault/$PROFILE/dlq/test_topic/msg1"

echo "📦 2. 运行 CLI 触发并激活 V3 自动迁移..."
# 使用 init 触发
"$COWEN_BIN" init --profile "$PROFILE" --app-key "dummykey" --app-secret "dummysecret" --app-mode self-built --certificate "dummy_cert" --encrypt-key "1234567890123456"

echo "📦 3. 校验迁移成果..."
# 3.1 校验 monolithic json 是否被重命名备份
if [ ! -f "vault/$PROFILE.json.v2_bak" ]; then
    fail_suite "Monolithic V2 json was not backed up"
fi

# 3.2 校验配置项是否已被拆分物理写入新位置 (vault/profile/cfg/key1)
if [ ! -f "vault/$PROFILE/cfg/key1" ]; then
    fail_suite "Config item 'key1' was not migrated to split file"
fi
# 读取它的内容，确认包含 val1
if ! grep -q '"value":"val1"' "vault/$PROFILE/cfg/key1"; then
    fail_suite "Migrated config value mismatch: expected val1 in file content"
fi

# 3.3 校验 tok_v2 是否被成功重命名为 tokens
if [ ! -d "vault/$PROFILE/tokens" ]; then
    fail_suite "tok_v2 directory was not renamed to tokens"
fi
if [ ! -f "vault/$PROFILE/tokens/access_token" ]; then
    fail_suite "tokens/access_token is missing"
fi

# 3.4 校验 dlq 结构是否被拍平 (移出子目录到 dlq/)
if [ ! -f "vault/$PROFILE/dlq/msg1" ]; then
    fail_suite "DLQ message was not migrated to dlq/ root"
fi

assert_pass "V2 to V3 Storage migration tested successfully"
echo "✅ Storage migration tests Passed!"
