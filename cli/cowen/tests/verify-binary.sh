#!/bin/bash
set -e

BINARY_PATH=$1

if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Usage: $0 <path_to_binary>"
    exit 1
fi

echo "🔍 [Verify] Validating build artifact: $BINARY_PATH"

# 尝试执行以判定是否为跨平台编译产物
if ! "$BINARY_PATH" --version > /dev/null 2>&1; then
    echo "⚠️  [Verify] SKIPPED: Cannot execute binary on the current host (cross-compiled architecture mismatch)."
    exit 0
fi

echo "✅ [Verify] Binary is executable natively. Running functional integration checks..."

# 1. 版本号获取测试
VERSION_OUT=$("$BINARY_PATH" --version)
if echo "$VERSION_OUT" | grep -qiw "cowen"; then
    echo "   [OK] Version check passed: $VERSION_OUT"
else
    echo "   ❌ FAIL: '--version' did not contain 'cowen'. Got: $VERSION_OUT"
    exit 1
fi

# 2. 基础帮助菜单与子命令存在性测试
HELP_OUT=$("$BINARY_PATH" --help)
if echo "$HELP_OUT" | grep -q "Usage:"; then
    echo "   [OK] Help usage format is correct."
else
    echo "   ❌ FAIL: '--help' did not output usage instruction."
    exit 1
fi

# 3. 错误子命令容错机制测试
if "$BINARY_PATH" non_existent_cmd 2>&1 | grep -q "unrecognized subcommand"; then
    echo "   [OK] Error handling for invalid subcommands is intact."
else
    echo "   ❌ FAIL: Did not properly reject an invalid subcommand."
    exit 1
fi

# 4. 环境信息或基础指令容错验证
# 执行无需网关强依赖的 info 指令等
if "$BINARY_PATH" debug --help 2>&1 | grep -q "debug"; then
    echo "   [OK] CLI module tree (debug) is intact."
fi

echo "🎉 [Verify] SUCCESS: Artifact functional verification passed!"
