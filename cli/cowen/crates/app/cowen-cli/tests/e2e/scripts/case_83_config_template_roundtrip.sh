#!/usr/bin/env bash
set -e

# Case 83: Config template export and roundtrip import E2E validation
# Verifies that config template correctly exports and uncomments configured
# gateway blocks and that the exported template can be re-imported without loss.

if [ -f "crates/app/cowen-cli/tests/e2e/scripts/common.sh" ]; then
    source crates/app/cowen-cli/tests/e2e/scripts/common.sh
else
    source "$(dirname "$0")/common.sh"
fi

setup_workspace "case_83"
trap cleanup_suite EXIT
start_mock

PORT_GW=$(get_unused_port)
PORT_PROXY=$(get_unused_port)

echo "=========================================================="
echo "2. [执行] 阶段 1: 创建具有复杂 gateway 配置的初始 Profile p1"
echo "=========================================================="

# 💡 将临时 yaml 写入 COWEN_HOME 之外的目录，避免被 Daemon 自动加载为同名 Profile
ORIG_YAML="$COWEN_HOME/../orig_template.yaml"
cat <<EOF > "$ORIG_YAML"
app_key: "key_roundtrip_test"
app_mode: "store-app"
webhook_target: "$MOCK_URL/webhook"
proxy_port: $PORT_PROXY
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:$PORT_GW"
  auth_sync_hook: "$MOCK_URL/sync"
  auth_routing:
    mode: "STRICT"
    bypass_rules:
      - "/v1/ping"
      - "/static/**"
    require_rules:
      - "**"
  routes:
    - path: "/open-api/**"
      upstream: "openapi"
      strip_prefix: "/open-api"
    - path: "/**"
      upstream: "$MOCK_URL"
EOF

echo ">> 正在初始化 profile 'p1'..."
"$COWEN_BIN" init --profile p1 \
    --file "$ORIG_YAML" \
    --app-secret "mysecret" \
    --encrypt-key "1234567890123456" >/dev/null

echo ">> 启动 daemon 并等待运行以激活配置..."
"$COWEN_BIN" daemon start --profile p1
wait_for_daemon p1 10

echo "=========================================================="
echo " [执行] 阶段 2: 导出配置模板并校验反注释内容"
echo "=========================================================="

EXPORTED_YAML="$COWEN_HOME/../exported_template.yaml"
"$COWEN_BIN" config template --profile p1 > "$EXPORTED_YAML"

echo ">> 正在对导出的模板文件进行 Grep 字段完整性校验..."
grep -q "^gateway:" "$EXPORTED_YAML" || fail_suite "gateway header should be uncommented"
grep -q "bind_address: \"127.0.0.1:$PORT_GW\"" "$EXPORTED_YAML" || fail_suite "bind_address should be pre-filled and uncommented"
grep -q "auth_sync_hook: \"$MOCK_URL/sync\"" "$EXPORTED_YAML" || fail_suite "auth_sync_hook should be pre-filled and uncommented"
grep -q "mode: \"STRICT\"" "$EXPORTED_YAML" || fail_suite "auth_routing mode should be pre-filled and uncommented"
grep -q "\- \"/v1/ping\"" "$EXPORTED_YAML" || fail_suite "bypass_rules /v1/ping should be present in exported template"
grep -q "\- \"/static/\*\*\"" "$EXPORTED_YAML" || fail_suite "bypass_rules /static/** should be present in exported template"
grep -q "upstream: openapi" "$EXPORTED_YAML" || fail_suite "routes upstream openapi should be present in exported template"
grep -q "upstream: $MOCK_URL" "$EXPORTED_YAML" || fail_suite "routes upstream $MOCK_URL should be present in exported template"

assert_pass "Grep verification on exported template completed successfully"

echo "=========================================================="
echo " [执行] 阶段 3: 将导出的模板重新导入并初始化为 profile p2"
echo "=========================================================="

echo ">> 正在停止并彻底重置 p1 配置以防止端口冲突..."
"$COWEN_BIN" daemon stop --profile p1 >/dev/null 2>&1 || true
"$COWEN_BIN" reset --profile p1 --no-telemetry >/dev/null 2>&1 || true

echo ">> 正在使用导出的模板初始化 profile 'p2'..."
"$COWEN_BIN" init --profile p2 \
    --file "$EXPORTED_YAML" \
    --app-secret "mysecret" \
    --encrypt-key "1234567890123456" >/dev/null

echo ">> 正在校验 profile 'p2' 导入后的配置内容..."
CFG=$("$COWEN_BIN" config --profile p2)

assert_match "$CFG" "key_roundtrip_test" "p2 has app_key key_roundtrip_test"
assert_match "$CFG" "bind_address: 127.0.0.1:$PORT_GW" "p2 has bind_address"
assert_match "$CFG" "auth_sync_hook: $MOCK_URL/sync" "p2 has auth_sync_hook"
assert_match "$CFG" "bypass_rules:" "p2 has bypass_rules"
assert_match "$CFG" "\- /v1/ping" "p2 has bypass_rule /v1/ping"
assert_match "$CFG" "\- /static/\*\*" "p2 has bypass_rule /static/**"
assert_match "$CFG" "upstream: openapi" "p2 has routes upstream openapi"
assert_match "$CFG" "upstream: $MOCK_URL" "p2 has routes upstream $MOCK_URL"

echo "✅ Case 83 Passed! Roundtrip configuration template import/export matches perfectly!"
