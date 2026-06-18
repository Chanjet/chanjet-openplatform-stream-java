#!/usr/bin/env bash
# case_84_signer_verification.sh
# Tests the full plug-in signing offline tool chain (cowen-signer)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_84_signer"
setup_workspace "case_84_$PROFILE"
cd "$COWEN_HOME"

COWEN_SIGNER_BIN="./cowen-signer"
if [ ! -f "$COWEN_SIGNER_BIN" ]; then
    fail_suite "cowen-signer binary not found at $COWEN_SIGNER_BIN"
fi

echo "🔑 1. 测试 GenerateRoot 生成根证书密钥对..."
"$COWEN_SIGNER_BIN" generate-root --out-root-key root.pk8 --out-root-pub root.pub

if [ ! -f "root.pk8" ] || [ ! -f "root.pub" ]; then
    fail_suite "Failed to generate root key files"
fi
assert_pass "GenerateRoot command executed successfully"

echo "🔑 2. 测试 IssueCert 颁发开发者证书..."
"$COWEN_SIGNER_BIN" issue-cert \
    --root-key root.pk8 \
    --dev-id "test-developer" \
    --out-dev-key dev.pk8 \
    --out-cert dev_cert.json \
    --days 30 \
    --org "TestOrg" \
    --country "CN"

if [ ! -f "dev.pk8" ] || [ ! -f "dev_cert.json" ]; then
    fail_suite "Failed to issue developer certificate"
fi

# 检查证书内容是否包含正确的开发者ID
if ! grep -q "test-developer" dev_cert.json; then
    fail_suite "Developer ID mismatch in certificate"
fi
assert_pass "IssueCert command executed successfully"

echo "🔑 3. 测试 SignPlugin 签署插件动态库包..."
# 创建 mock 的 dylib 临时文件
echo "mock_dylib_bytes" > dummy.dylib
# 创建 mock 的 plugin.json
cat > plugin.json <<EOF
{
  "required_capabilities": {
    "auth": {}
  },
  "requested_permissions": {
    "storage": {}
  },
  "transport": "uds"
}
EOF

"$COWEN_SIGNER_BIN" sign-plugin \
    --dylib dummy.dylib \
    --name "mock-plugin" \
    --version "1.0.0" \
    --dev-key dev.pk8 \
    --dev-cert dev_cert.json \
    --out-bundle signature.bundle \
    --manifest-file plugin.json

if [ ! -f "signature.bundle" ]; then
    fail_suite "Failed to generate signed plugin signature.bundle"
fi

# 校验签名包是否包含证书与 manifest 信息
if ! grep -q "mock-plugin" signature.bundle || ! grep -q "test-developer" signature.bundle; then
    fail_suite "Invalid details in signature bundle"
fi
assert_pass "SignPlugin command executed successfully"

echo "✅ Signer Offline Toolchain tests Passed!"
