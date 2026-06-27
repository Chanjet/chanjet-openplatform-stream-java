#!/usr/bin/env bash
# case_89_store_migration_cli.sh
# Tests store migration through CLI (cowen store migrate) to SQLite and validates configuration/secrets are successfully transferred and app.yaml is updated.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_89_mig"
setup_workspace "case_89_$PROFILE"
cd "$COWEN_HOME"

# 覆盖默认 app.yaml 以激活 local 存储引擎以便触发迁移
cat > "app.yaml" <<EOF
storage:
  store: local
log:
  level: debug
EOF

echo "📦 1. 物理配置 local 存储并写入测试数据..."
# 运行 init 自动配置账户
"$COWEN_BIN" init --profile "$PROFILE" --app-key "dummykey" --app-secret "dummysecret" --app-mode self-built --certificate "dummy_cert" --encrypt-key "1234567890123456"

# 我们可以通过 CLI 写入一些配置数据
"$COWEN_BIN" config set --profile "$PROFILE" webhook_target "http://localhost:9999"

echo "📦 2. 启动 Daemon 服务端..."
"$COWEN_BIN" daemon start --profile "$PROFILE" >/dev/null
wait_for_daemon "$PROFILE" 10

echo "📦 3. 运行 Store Migrate CLI 命令..."
# 迁移到 sqlite
TARGET_DB="sqlite://test_migrate_89.db"

"$COWEN_BIN" store migrate --to "$TARGET_DB" --mode clone

echo "📦 4. 校验迁移结果..."

# 4.1 校验 app.yaml 存储驱动是否切换为 innerdb
if ! grep -q 'store: innerdb' "app.yaml"; then
    fail_suite "app.yaml was not updated with store: innerdb"
fi

if ! grep -q "db_url: sqlite://test_migrate_89.db" "app.yaml"; then
    fail_suite "app.yaml was not updated with db_url"
fi

# 4.2 校验新生成的 sqlite 数据库文件中是否有迁移的配置数据。
# 停止 daemon，因为 app.yaml 已经改写为 innerdb，重新启动 daemon 会加载 SQLite 存储
kill_daemons_in_dirs "$COWEN_HOME"
sleep 3

# 重新启动 Daemon (现在它应该加载 sqlite 存储后端了)
"$COWEN_BIN" daemon start --profile "$PROFILE" >/dev/null
wait_for_daemon "$PROFILE" 10

# 用 CLI 去 get config，如果能读到迁移后的值证明迁移成功！
MIGRATED_VAL=$("$COWEN_BIN" config get webhook_target --profile "$PROFILE" 2>/dev/null || true)
if [ "$MIGRATED_VAL" != "http://localhost:9999" ]; then
    echo "❌ Debug config get result: $MIGRATED_VAL"
    fail_suite "Migrated config value webhook_target mismatch: expected 'http://localhost:9999', got '$MIGRATED_VAL'"
fi

# 清理 daemon
kill_daemons_in_dirs "$COWEN_HOME"

assert_pass "Store migration through CLI tested successfully"
echo "✅ Store migration CLI tests Passed!"
