#!/usr/bin/env bash
# case_91_cli_api_commands.sh
# Tests CLI api commands (list, spec) and their formats.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

setup_workspace "case_91"
trap cleanup_suite EXIT

PROFILE="api_test"

echo "📦 1. 初始化 profile 并启动守护进程..."
"$COWEN_BIN" init --profile "$PROFILE" \
    --app-key "dummy_app_key" \
    --app-secret "dummy_app_secret" \
    --app-mode self-built \
    --certificate "dummy_cert" \
    --encrypt-key "1234567890123456"

"$COWEN_BIN" daemon start --profile "$PROFILE" >/dev/null
wait_for_daemon "$PROFILE" 10

echo "📦 2. 运行 cowen api list 相关指令..."
"$COWEN_BIN" api list --profile "$PROFILE"
"$COWEN_BIN" api list --profile "$PROFILE" --format json
"$COWEN_BIN" api list --profile "$PROFILE" --format yaml
"$COWEN_BIN" api list --profile "$PROFILE" --search "token"

echo "📦 3. 运行 cowen api spec 相关指令..."
"$COWEN_BIN" api spec GET /dummy --profile "$PROFILE" --raw || true
"$COWEN_BIN" api spec GET /dummy --profile "$PROFILE" || true

kill_daemons_in_dirs "$COWEN_HOME"

assert_pass "CLI API commands tested successfully"
echo "✅ CLI API commands E2E tests Passed!"
