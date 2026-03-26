.PHONY: build build-test build-mac build-windows build-linux build-all clean mkdir-build

# 生产环境默认值 (与代码中一致)
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com

# 测试环境默认值
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

# 变量路径
CONFIG_PKG = cjtCli/internal/core/config

LDFLAGS_PROD = -X $(CONFIG_PKG).DefaultOpenApiURL=$(PROD_OPENAPI) -X $(CONFIG_PKG).DefaultStreamURL=$(PROD_STREAM)
LDFLAGS_TEST = -X $(CONFIG_PKG).DefaultOpenApiURL=$(TEST_OPENAPI) -X $(CONFIG_PKG).DefaultStreamURL=$(TEST_STREAM)

# Tokenizers 底层静态库路径 (跨平台 CGO 链接依赖)
LIB_DIR = $(PWD)/pkg/search/assets/lib

# 默认构建 (生产环境 - 依赖当前系统默认 CGO 环境)
build:
	CGO_ENABLED=1 go build -ldflags "$(LDFLAGS_PROD)" -o cjtCli main.go

# 测试环境构建 (CGO 启用且带有本地链接路径)
build-test:
	CGO_ENABLED=1 CGO_LDFLAGS="-L$(LIB_DIR)/darwin-arm64 -L$(PWD)/lib" go build -ldflags "$(LDFLAGS_TEST)" -o cjtCli-test main.go

# --------- 跨平台一键编译组 (生成的二进制统一输出至 build/) ---------

mkdir-build:
	mkdir -p build

build-mac: mkdir-build
	@echo "🍎 Building macOS (arm64)..."
	GOOS=darwin GOARCH=arm64 CGO_ENABLED=1 \
	CGO_LDFLAGS="-L$(LIB_DIR)/darwin-arm64" \
	go build -ldflags "$(LDFLAGS_PROD)" -o build/cjtCli-darwin-arm64 main.go

build-windows: mkdir-build
	@echo "🪟 Building Windows (amd64) via MingW CGO..."
	@echo "   Note: Requires 'brew install mingw-w64' on macOS."
	GOOS=windows GOARCH=amd64 CGO_ENABLED=1 \
	CC=x86_64-w64-mingw32-gcc \
	CXX=x86_64-w64-mingw32-g++ \
	CGO_LDFLAGS="-L$(LIB_DIR)/windows-amd64 -lws2_32 -lbcrypt -luserenv -ladvapi32 -lntdll" \
	go build -ldflags "$(LDFLAGS_PROD)" -o build/cjtCli-windows-amd64.exe main.go

build-linux: mkdir-build
	@echo "🐧 Building Linux (amd64) CGO..."
	@echo "   Note: Best executed within a Native Linux / Docker environment."
	GOOS=linux GOARCH=amd64 CGO_ENABLED=1 \
	CGO_LDFLAGS="-L$(LIB_DIR)/linux-amd64" \
	go build -ldflags "$(LDFLAGS_PROD)" -o build/cjtCli-linux-amd64 main.go

build-all-prod: build-mac build-windows build-linux
	@echo "✅ All production binaries targeted... Check ./build/"

clean:
	rm -f cjtCli cjtCli-test
	rm -rf build/
