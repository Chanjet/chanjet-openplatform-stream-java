#!/bin/bash
set -e

# 配置路径
ASSETS_DIR="cli/cjtCli/pkg/search/assets"
MODEL_DIR="$ASSETS_DIR/models"
LIB_BASE="$ASSETS_DIR/lib"

echo "🚀 [AI 补能] 正在准备 cjtCli 的真·语义搜索资产..."

# 1. 下载 BGE-Micro-v2 语义模型 (约 30MB)
echo "📥 正在下载语义向量模型 (BGE-Micro-v2)..."
curl -L "https://huggingface.co/Xenova/bge-micro-v2/resolve/main/onnx/model.onnx" -o "$MODEL_DIR/bge-micro-v2.onnx"
curl -L "https://huggingface.co/Xenova/bge-micro-v2/resolve/main/tokenizer.json" -o "$MODEL_DIR/tokenizer.json"

# 2. 下载 ONNX 运行时动态库 (根据当前平台)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

if [ "$OS" == "darwin" ]; then
    if [ "$ARCH" == "arm64" ]; then
        PLATFORM="darwin-arm64"
        URL="https://github.com/microsoft/onnxruntime/releases/download/v1.17.1/onnxruntime-osx-arm64-1.17.1.tgz"
        LIB_NAME="libonnxruntime.1.17.1.dylib"
        TARGET_LIB="libonnxruntime.dylib"
    else
        PLATFORM="darwin-x64"
        URL="https://github.com/microsoft/onnxruntime/releases/download/v1.17.1/onnxruntime-osx-x86_64-1.17.1.tgz"
        LIB_NAME="libonnxruntime.1.17.1.dylib"
        TARGET_LIB="libonnxruntime.dylib"
    fi
elif [ "$OS" == "linux" ]; then
    PLATFORM="linux-amd64"
    URL="https://github.com/microsoft/onnxruntime/releases/download/v1.17.1/onnxruntime-linux-x64-1.17.1.tgz"
    LIB_NAME="libonnxruntime.so.1.17.1"
    TARGET_LIB="libonnxruntime.so"
else
    echo "❌ 暂不支持自动下载当前平台的运行时，请手动下载 onnxruntime.dll 并放入 $LIB_BASE/windows-amd64/"
    exit 1
fi

echo "📥 正在下载 $PLATFORM 平台的 AI 运行时库..."
TMP_DIR=$(mktemp -d)
curl -L "$URL" -o "$TMP_DIR/ort.tgz"
tar -xzf "$TMP_DIR/ort.tgz" -C "$TMP_DIR"

# 提取库文件到对应目录
mkdir -p "$LIB_BASE/$PLATFORM"
find "$TMP_DIR" -name "$LIB_NAME" -exec cp {} "$LIB_BASE/$PLATFORM/$TARGET_LIB" \;

echo "✅ [AI 补能] 资产就绪！"
echo "   - 模型: $MODEL_DIR/bge-micro-v2.onnx"
echo "   - 库: $LIB_BASE/$PLATFORM/$TARGET_LIB"
echo ""
echo "🔥 现在请运行 'go build -o cjtCli cli/cjtCli/main.go' 重新编译应用。"
