# cjtCli - Open Streaming Connector CLI

cjtCli 是 Open Streaming Connector 的命令行客户端，支持完整的系统治理、API 调用模拟以及基于大语言模型的本地语义搜索功能。

## 环境要求

本项目核心依赖以下组件实现高性能 AI 搜索（基于 ONNX Runtime 与 HuggingFace Tokenizers）：
- Go 1.21+
- [CGO 启动] 构建时必须开启 `CGO_ENABLED=1`
- [跨平台预热库] 底层 Rust 分词库 (`libtokenizers.a`) 需事先存放于 `pkg/search/assets/lib/<OS-ARCH>/` 中

---

## 构建指南 (Build Instructions)

本项目强依赖 CGO (因包含 C/C++ 与 Rust 库)。以下是各平台的本地与交叉编译构建指南：

### 1. macOS (原生编译 - Apple Silicon)

若您在 macOS 宿主机上开发，直接执行标准 go build 即可（依赖系统自带的 Clang）：

```bash
cd cli/cjtCli
CGO_LDFLAGS="-L$(pwd)/pkg/search/assets/lib/darwin-arm64" go build -o build/cjtCli-darwin-arm64 ./cmd/cjtCli
```

### 2. Windows amd64 (在 macOS 上交叉编译)

由于包含了非 Go 写的底层依赖（ONNX C 接口与 Rust 库），从 macOS 交叉编译到 Windows 需要特殊绕行：

1. **安装 MingW 工具链**：`brew install mingw-w64`
2. **利用存根与系统级补充**：
   - 依赖的静态库 `libtokenizers.a`（Windows amd64 版）必须已置于 `pkg/search/assets/lib/windows-amd64/`
   - 为绕过 `tokenizers` 依赖硬编码的 `-ldl`，目录内包含一个 `libdl.a` 空存根文件。
   - 必须向 MingW 链接器补充注入 Windows 网络与安全库：`-lws2_32 -lbcrypt -luserenv -ladvapi32 -lntdll`

**一键交叉编译命令**：
```bash
GOOS=windows GOARCH=amd64 CGO_ENABLED=1 \
CC=x86_64-w64-mingw32-gcc \
CXX=x86_64-w64-mingw32-g++ \
CGO_LDFLAGS="-L$(pwd)/pkg/search/assets/lib/windows-amd64 -lws2_32 -lbcrypt -luserenv -ladvapi32 -lntdll" \
go build -o build/cjtCli-windows-amd64.exe ./cmd/cjtCli
```

### 3. Linux amd64 (在目标机/Docker内环境编译)

不要在 macOS 宿主机直连编译 Linux CGO。最佳实践是在 Linux 机器或 `golang:1.21-bullseye` Docker 容器内执行：

```bash
# 确保 linux-amd64 的 libtokenizers.a 静态库已在约定目录内
CGO_LDFLAGS="-L$(pwd)/pkg/search/assets/lib/linux-amd64" go build -o build/cjtCli-linux-amd64 ./cmd/cjtCli
```

---

## 依赖更新须知 (Rust 预热)

当前工程（截至 `tokenizers v1.26.0`）已经实现了 **“一次预热，处处编译”**，底层静态库 `.a` 已经提交在代码树中。
**后续日常的 `go build` 完全不需要再去关注 Rust 或执行 Cargo。**

*仅当未来升级* `github.com/daulet/tokenizers` 依赖版本时，才需要：
1. 重新进入 CGO 环境中生成新的 `libtokenizers.a`
2. 或者去 [daulet/tokenizers Releases](https://github.com/daulet/tokenizers/releases) 直接下载对应平台的 `.tar.gz` 预编译静态库塞入 `assets/lib/` 目录中。
