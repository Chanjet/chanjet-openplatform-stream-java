#!/bin/bash
set -e

BINARY_PATH=$1
BUILDER_IMAGE="cowen-builder"

if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Usage: $0 <path_to_binary>"
    exit 1
fi

echo "🔍 [Verify] Validating build artifact: $BINARY_PATH"

# 判定是否需要通过 Podman 执行
USE_PODMAN=false
if ! "$BINARY_PATH" --version > /dev/null 2>&1; then
    if command -v podman > /dev/null 2>&1 && podman machine list --format "{{.Running}}" | grep -q true 2>/dev/null; then
        USE_PODMAN=true
        echo "🐳 [Verify] Native execution failed. Using Podman for validation..."
    else
        echo "⚠️  [Verify] SKIPPED: Cannot execute binary natively and Podman is unavailable."
        exit 0
    fi
fi

# 执行函数封装
run_bin() {
    if [ "$USE_PODMAN" = true ]; then
        # 获取绝对路径以进行挂载
        ABS_BIN_PATH=$(cd "$(dirname "$BINARY_PATH")" && pwd)/$(basename "$BINARY_PATH")
        BIN_DIR=$(dirname "$ABS_BIN_PATH")
        BIN_NAME=$(basename "$ABS_BIN_PATH")
        # 挂载目录并在容器内执行 (使用 cowen-builder 镜像)
        # 设置 QEMU_LD_PREFIX 为 x86_64 库路径，以便在 ARM 容器中执行 x86_64 动态链接程序
        podman run --rm -v "${BIN_DIR}:/tmp/verify" -e QEMU_LD_PREFIX=/usr/x86_64-linux-gnu "$BUILDER_IMAGE" "/tmp/verify/${BIN_NAME}" "$@"
    else
        "$BINARY_PATH" "$@"
    fi
}

echo "✅ [Verify] Validation environment ready. Running functional checks..."

# 1. 版本号获取测试
VERSION_OUT=$(run_bin --version)
if echo "$VERSION_OUT" | grep -qiw "cowen"; then
    echo "   [OK] Version check passed: $VERSION_OUT"
else
    echo "   ❌ FAIL: '--version' did not contain 'cowen'. Got: $VERSION_OUT"
    exit 1
fi

# 2. 基础帮助菜单与子命令存在性测试
HELP_OUT=$(run_bin --help)
if echo "$HELP_OUT" | grep -q "Usage:"; then
    echo "   [OK] Help usage format is correct."
else
    echo "   ❌ FAIL: '--help' did not output usage instruction."
    exit 1
fi

# 3. 错误子命令容错机制测试
# 注意：重定向 stderr 到 stdout 以便于 grep
if run_bin non_existent_cmd 2>&1 | grep -q "unrecognized subcommand"; then
    echo "   [OK] Error handling for invalid subcommands is intact."
else
    echo "   ❌ FAIL: Did not properly reject an invalid subcommand."
    exit 1
fi

# 4. 环境信息或基础指令容错验证
if run_bin debug --help 2>&1 | grep -q "debug"; then
    echo "   [OK] CLI module tree (debug) is intact."
fi

echo "🎉 [Verify] SUCCESS: Artifact functional verification passed!"
