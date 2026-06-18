#!/bin/bash
set -e

FILE_PATH=$1

if [ -z "$FILE_PATH" ] || [ ! -f "$FILE_PATH" ]; then
    echo "❌ Usage: $0 <path_to_artifact>"
    exit 1
fi

echo "🔍 [Verify SHA256] Validating checksum files for: $FILE_PATH"

# 1. 检查是否存在 .sha1 校验文件（预期被废弃）
SHA1_PATH="${FILE_PATH}.sha1"
if [ -f "$SHA1_PATH" ]; then
    echo "❌ FAIL: Obsolete SHA1 checksum file still exists: $SHA1_PATH"
    exit 1
else
    echo "   [OK] Obsolete SHA1 checksum file is absent."
fi

# 2. 检查是否存在 .sha256 校验文件
SHA256_PATH="${FILE_PATH}.sha256"
if [ ! -f "$SHA256_PATH" ]; then
    echo "❌ FAIL: SHA256 checksum file does not exist: $SHA256_PATH"
    exit 1
else
    echo "   [OK] SHA256 checksum file exists."
fi

# 3. 读取并检查校验文件的格式（必须是 64 字符的十六进制哈希值）
CHECKSUM_CONTENT=$(cat "$SHA256_PATH" | tr -d '[:space:]')
if [[ ! "$CHECKSUM_CONTENT" =~ ^[a-fA-F0-9]{64}$ ]]; then
    echo "❌ FAIL: Invalid SHA256 checksum format. Expected 64 hex characters, got: '$CHECKSUM_CONTENT'"
    exit 1
else
    echo "   [OK] SHA256 checksum file format is valid."
fi

# 4. 在本地环境计算实际的 sha256，进行内容一致性校验
if command -v shasum >/dev/null 2>&1; then
    ACTUAL_HASH=$(shasum -a 256 "$FILE_PATH" | cut -d ' ' -f 1 | tr -d '[:space:]')
elif command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH=$(sha256sum "$FILE_PATH" | cut -d ' ' -f 1 | tr -d '[:space:]')
else
    echo "⚠️  [Verify SHA256] Skip actual hash matching because neither shasum nor sha256sum is installed."
    exit 0
fi

# 转换为小写比对
ACTUAL_HASH_LOWER=$(echo "$ACTUAL_HASH" | tr '[:upper:]' '[:lower:]')
CHECKSUM_CONTENT_LOWER=$(echo "$CHECKSUM_CONTENT" | tr '[:upper:]' '[:lower:]')

if [ "$ACTUAL_HASH_LOWER" != "$CHECKSUM_CONTENT_LOWER" ]; then
    echo "❌ FAIL: Checksum mismatch!"
    echo "   Actual SHA256 of file: $ACTUAL_HASH_LOWER"
    echo "   Content in .sha256:     $CHECKSUM_CONTENT_LOWER"
    exit 1
else
    echo "   [OK] Checksum verification matched: $ACTUAL_HASH_LOWER"
fi

echo "🎉 [Verify SHA256] SUCCESS: Checksum validation passed!"
exit 0
