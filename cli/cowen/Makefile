BINARY=cowen
PKG=cli/cowen
OUTPUT_DIR=../../bin

# 从 Cargo.toml 自动提取版本号
ifeq ($(OS),Windows_NT)
    VERSION := $(shell powershell -NoProfile -Command "(Get-Content crates/cowen-cli/Cargo.toml | Select-String '^version =').ToString().Split('\"')[1].Trim()")
else
    VERSION := $(shell grep '^version =' crates/cowen-cli/Cargo.toml | cut -d '"' -f 2)
endif

# 缓存目录与镜像定义
CARGO_CACHE_DIR=$(HOME)/.cache/cowen-cargo
BUILDER_IMAGE=localhost/cowen-builder
MAX_PARALLEL ?= 1

# 检测当前主机环境
ifeq ($(OS),Windows_NT)
    HOST_OS := windows
    HOST_ARCH := x86_64
else
    UNAME_S := $(shell uname -s)
    UNAME_M := $(shell uname -m)
    ifeq ($(UNAME_S),Darwin)
        HOST_OS := macos
    else
        HOST_OS := linux
    endif
    ifeq ($(UNAME_M),x86_64)
        HOST_ARCH := x86_64
    else ifeq ($(UNAME_M),arm64)
        HOST_ARCH := aarch64
    else ifeq ($(UNAME_M),aarch64)
        HOST_ARCH := aarch64
    else
        HOST_ARCH := $(UNAME_M)
    endif
endif
# --- 本地开发编译提速优化 (Local Fast Build Optimizations) ---
# 如果不在 CI 环境下，且没有显式禁用，并且当前执行的命令中不包含 'package'，默认关闭 LTO 并恢复多线程代码生成以大幅提升本地编译速度
ifndef CI
ifndef DISABLE_FAST_BUILD
ifeq ($(findstring package,$(MAKECMDGOALS)),)
    export CARGO_PROFILE_RELEASE_LTO ?= false
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS ?= 256
    
    # 自动探测 sccache 并启用
    SCCACHE_EXISTS := $(shell command -v sccache 2> /dev/null)
    ifdef SCCACHE_EXISTS
        export RUSTC_WRAPPER = sccache
    endif
endif
endif
endif

# --- 跨平台兼容性宏 ---
ifeq ($(OS),Windows_NT)
    RM = powershell -NoProfile -Command "$(foreach d,$(1),if(Test-Path $(d)){ Remove-Item -Recurse -Force $(d) };)"
    MD5 = powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm MD5 $(1)).Hash.ToLower(); Set-Content -Path $(1).md5 -Value $$hash"
    SHA1 = powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm SHA1 $(1)).Hash.ToLower(); Set-Content -Path $(1).sha1 -Value $$hash"
else
    RM = rm -rf $(1)
    MD5 = (md5 -q $(1) 2>/dev/null || md5sum $(1) | cut -d ' ' -f 1) > $(1).md5
    SHA1 = (shasum -a 1 $(1) 2>/dev/null || sha1sum $(1)) | cut -d ' ' -f 1 > $(1).sha1
endif

VERIFY_BIN = ./crates/cowen-cli/tests/e2e/scripts/verify-binary.sh $(1)

# 生产/测试环境配置
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build macos-aarch64 linux-x86_64 linux-x86_64-with-podman linux-aarch64 windows-x86_64 windows-x86-cross windows-x86_64-cross \
        windows-plugin-x64 windows-plugin-x86 test-macos-aarch64 \
        test test-linux test-win test-macos check-cross \
        package package-macos-aarch64 package-linux-x86_64 package-linux-x86_64-with-podman \
        package-linux-aarch64 package-windows-x86_64 package-windows-x86_64-cross package-windows-x86-cross clean install run prepare-podman-image prepare-podman-machine stop-podman-machine

# --- 默认构建目标 (Default Build Target) ---
build:
ifeq ($(HOST_OS),macos)
	@$(MAKE) macos-aarch64
else ifeq ($(HOST_OS),linux)
ifeq ($(HOST_ARCH),x86_64)
	@$(MAKE) linux-x86_64
else
	@$(MAKE) linux-aarch64
endif
else ifeq ($(HOST_OS),windows)
	@$(MAKE) windows-x86_64
endif

# --- 默认打包目标 (Default Package Target) ---
package:
ifeq ($(HOST_OS),macos)
	@$(MAKE) package-macos-aarch64
else ifeq ($(HOST_OS),linux)
ifeq ($(HOST_ARCH),x86_64)
	@$(MAKE) package-linux-x86_64
else
	@$(MAKE) package-linux-aarch64
endif
else ifeq ($(HOST_OS),windows)
	@$(MAKE) package-windows-x86_64
endif

# 授权配置 (Authentication)
OFFICIAL_APP_KEY = 3NWdEbmu    # 正式环境 AppKey (Production)
PREVIEW_APP_KEY  ?= RjP3LPoI   # 预览/集成测试环境 AppKey (Preview/INTE)

# --- 核心构建目标 ---

build-plugins:
	@echo "🛠️ Building search-embedding plugin (Release)..."
	cargo build --release -p cowen-search-embedding
	@echo "🛠️ Building mcp-plugin (Release)..."
	cargo build --release -p cowen-mcp-plugin
	@echo "🔐 Signing plugins natively..."
ifeq ($(OS),Windows_NT)
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/release/libcowen_search_embedding.exe') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/libcowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy } }"
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/release/cowen-mcp-plugin.exe') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/cowen-mcp-plugin.exe --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/cowen-mcp-plugin.bundle --capabilities core.rpc.stdio --required-privileges LocalCacheAccess } }"
else
	@if [ -f "dist_assets/keys/official_dev.pk8" ]; then \
		if [ -f "target/release/libcowen_search_embedding" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/libcowen_search_embedding --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/libcowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy; fi; \
		if [ -f "target/release/cowen-mcp-plugin" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/cowen-mcp-plugin --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/cowen-mcp-plugin.bundle --capabilities core.rpc.stdio --required-privileges LocalCacheAccess; fi; \
	fi
endif

macos-aarch64: build-plugins
	@echo "🍎 Building macOS Apple Silicon v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/macos-aarch64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo build --release -p cowen-cli -p cowen-daemon
	cp target/release/$(BINARY) $(OUTPUT_DIR)/macos-aarch64/$(BINARY)
	cp target/release/cowen-daemon $(OUTPUT_DIR)/macos-aarch64/cowen-daemon
	cp target/release/libcowen_search_embedding $(OUTPUT_DIR)/macos-aarch64/ || true
	if [ -f "$(OUTPUT_DIR)/macos-aarch64/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle $(OUTPUT_DIR)/macos-aarch64/ || exit 1; fi
	cp target/release/cowen-mcp-plugin $(OUTPUT_DIR)/macos-aarch64/ || true
	if [ -f "$(OUTPUT_DIR)/macos-aarch64/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/macos-aarch64/ || exit 1; fi
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-aarch64/$(BINARY) 2>/dev/null || true
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-aarch64/cowen-daemon 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))

test-macos-aarch64:
	@echo "🧪 Building macOS Apple Silicon (TEST ENV) v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/test
	CARGO_BIN_NAME_OVERRIDE=$(BINARY)-test APP_DIR_NAME=.$(BINARY)-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) BUILTIN_CLIENT_ID=$(PREVIEW_APP_KEY) cargo build --release --features inte -p cowen-cli -p cowen-daemon
	cp target/release/$(BINARY) $(OUTPUT_DIR)/test/$(BINARY)
	cp target/release/cowen-daemon $(OUTPUT_DIR)/test/cowen-daemon
	@codesign --force --deep --sign - $(OUTPUT_DIR)/test/$(BINARY) 2>/dev/null || true
	@codesign --force --deep --sign - $(OUTPUT_DIR)/test/cowen-daemon 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/test/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/test/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/test/$(BINARY))


linux-x86_64: build-plugins
	@echo "🐧 Building Linux x86_64 [FULL VERSION] natively v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-x86_64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo build --release --target x86_64-unknown-linux-gnu -p cowen-cli -p cowen-daemon
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(OUTPUT_DIR)/linux-x86_64/$(BINARY)
	cp target/x86_64-unknown-linux-gnu/release/cowen-daemon $(OUTPUT_DIR)/linux-x86_64/cowen-daemon
	cp target/release/libcowen_search_embedding $(OUTPUT_DIR)/linux-x86_64/ || true
	if [ -f "$(OUTPUT_DIR)/linux-x86_64/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle $(OUTPUT_DIR)/linux-x86_64/ || exit 1; fi
	cp target/release/cowen-mcp-plugin $(OUTPUT_DIR)/linux-x86_64/ || true
	if [ -f "$(OUTPUT_DIR)/linux-x86_64/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/linux-x86_64/ || exit 1; fi
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))

# 确保 Podman 虚拟机已启动并响应 (仅 macOS/Windows)
prepare-podman-machine:
ifeq ($(HOST_OS),macos)
	@if ! podman machine list --format '{{.Default}} {{.Running}}' 2>/dev/null | grep -q '^true true'; then \
		echo "🚀 Starting Podman machine..."; \
		podman machine start || true; \
		echo "⌛ Waiting for Podman socket to be ready..."; \
		sleep 5; \
	fi
	@if ! podman ps > /dev/null 2>&1; then \
		echo "⚠️  Podman is running but not responsive. Attempting to set DOCKER_HOST..."; \
		export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null); \
		if [ -z "$$DOCKER_HOST" ]; then export DOCKER_HOST="unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"; fi; \
		echo "   Using DOCKER_HOST=$$DOCKER_HOST"; \
	fi
endif

# 停止 Podman 虚拟机 (仅 macOS)
stop-podman-machine:
ifeq ($(HOST_OS),macos)
	@echo "🛑 Stopping Podman machine to recycle resources..."
	@podman machine stop 2>/dev/null || true
endif

# 哨兵文件：记录镜像最后一次成功构建的时间
.builder_image_built: Dockerfile.builder
	@echo "🛠️ Building/Updating custom builder image $(BUILDER_IMAGE) (Native/Cross)..."
	podman build -t $(BUILDER_IMAGE) -f Dockerfile.builder .
	@touch .builder_image_built

# 准备 Podman 编译镜像
# 如果 Dockerfile.builder 没变且镜像已存在，则跳过
prepare-podman-image: prepare-podman-machine
	@if [ ! -f .builder_image_built ] || ! podman image exists $(BUILDER_IMAGE); then \
		$(MAKE) .builder_image_built; \
	fi

# --- Podman 共享执行宏 (Shared Podman Invocation) ---
# 定义统一的挂载与环境变量，确保打包与测试复用同一套缓存与构建产物
# 增加 --memory 与 --shm-size 确保容器有足够的运行资源，防止 OOM
PODMAN_RUN_BASE = podman run --rm \
	--memory=12g \
	--shm-size=2g \
	-v "$$(pwd)/../..:/workspace" \
	-v "$(CARGO_CACHE_DIR)/registry:/root/.cargo/registry" \
	-v "$(CARGO_CACHE_DIR)/git:/root/.cargo/git" \
	-v "$(CARGO_CACHE_DIR)/target:/workspace/cli/cowen/target_podman" \
	-w /workspace/cli/cowen \
	-e CARGO_TARGET_DIR=target_podman \
	-e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
	-e BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	-e DEF_OPENAPI_URL=$(PROD_OPENAPI) \
	-e DEF_STREAM_URL=$(PROD_STREAM) \
	-e CARGO_BIN_NAME_OVERRIDE=$(BINARY) \
	-e APP_DIR_NAME=.$(BINARY) \
	-e CARGO_PROFILE_RELEASE_LTO=false \
	-e CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256

# 提速版 Podman 构建 (系统依赖 + Rust 依赖双缓存)
linux-x86_64-with-podman: prepare-podman-image
	@echo "🐧 Building Linux x86_64 [FULL VERSION] via Podman v$(VERSION)..."
	@mkdir -p $(OUTPUT_DIR)/linux-x86_64
	@mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git $(CARGO_CACHE_DIR)/target
	@trap '$(MAKE) stop-podman-machine' EXIT; \
	$(PODMAN_RUN_BASE) \
		$(BUILDER_IMAGE) \
		bash -c "cargo build --release -j $(MAX_PARALLEL) --target x86_64-unknown-linux-gnu -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-signer && \
			target_podman/x86_64-unknown-linux-gnu/release/cowen-signer sign-plugin --dylib target_podman/x86_64-unknown-linux-gnu/release/libcowen_search_embedding --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target_podman/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy && \
			cp target_podman/x86_64-unknown-linux-gnu/release/$(BINARY) ../../bin/linux-x86_64/$(BINARY) && \
			cp target_podman/x86_64-unknown-linux-gnu/release/cowen-daemon ../../bin/linux-x86_64/cowen-daemon && \
			cp target_podman/x86_64-unknown-linux-gnu/release/libcowen_search_embedding ../../bin/linux-x86_64/ && \
			cp target_podman/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle ../../bin/linux-x86_64/ && \
			cp target_podman/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin ../../bin/linux-x86_64/ && \
			cp target_podman/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin.bundle ../../bin/linux-x86_64/" && \
	$(call MD5,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call SHA1,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))

linux-aarch64: build-plugins
	@echo "🐧 Building Linux aarch64 v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-aarch64
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo zigbuild --release --target aarch64-unknown-linux-musl --no-default-features -p cowen-cli -p cowen-daemon
	cp target/aarch64-unknown-linux-musl/release/$(BINARY) $(OUTPUT_DIR)/linux-aarch64/$(BINARY)
	cp target/aarch64-unknown-linux-musl/release/cowen-daemon $(OUTPUT_DIR)/linux-aarch64/cowen-daemon
	cp target/release/libcowen_search_embedding $(OUTPUT_DIR)/linux-aarch64/ || true
	if [ -f "$(OUTPUT_DIR)/linux-aarch64/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle $(OUTPUT_DIR)/linux-aarch64/ || exit 1; fi
	cp target/release/cowen-mcp-plugin $(OUTPUT_DIR)/linux-aarch64/ || true
	if [ -f "$(OUTPUT_DIR)/linux-aarch64/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/linux-aarch64/ || exit 1; fi
	@$(call MD5,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))

windows-x86_64: build-plugins
	@echo "🪟 Building Windows x86_64 v$(VERSION)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "$$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; $$env:BUILTIN_CLIENT_ID='$(OFFICIAL_APP_KEY)'; cargo build --release --target x86_64-pc-windows-msvc -p cowen-cli -p cowen-daemon"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/$(BINARY).exe -Destination $(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/cowen-daemon.exe -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-daemon.exe -Force"
	powershell -NoProfile -Command "if (Test-Path target/release/cowen_search_embedding.dll) { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/cowen_search_embedding.dll --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/cowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy; Copy-Item -Path target/release/cowen_search_embedding.dll -Destination $(OUTPUT_DIR)/windows-x86_64/cowen_search_embedding.dll -Force; Copy-Item -Path target/release/cowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/cowen_search_embedding.bundle -Force }"
	powershell -NoProfile -Command "if (Test-Path target/release/cowen-mcp-plugin.exe) { Copy-Item -Path target/release/cowen-mcp-plugin.exe -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-mcp-plugin.exe -Force; Copy-Item -Path target/release/cowen-mcp-plugin.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-mcp-plugin.bundle -Force }"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)

windows-x86-cross:
	@echo "🪟 Cross-compiling Windows x86 (32-bit) v$(VERSION) via cargo-zigbuild..."
	mkdir -p $(OUTPUT_DIR)/windows-x86
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
	DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	cargo zigbuild --release --target i686-pc-windows-gnu --no-default-features -p cowen-cli -p cowen-daemon
	cp target/i686-pc-windows-gnu/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x86/$(BINARY).exe
	cp target/i686-pc-windows-gnu/release/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86/cowen-daemon.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x86/$(BINARY).exe)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86/$(BINARY).exe)

windows-x86_64-cross:
	@echo "🪟 Cross-compiling Windows x86_64 (64-bit) v$(VERSION) via cargo-zigbuild..."
	mkdir -p $(OUTPUT_DIR)/windows-x86_64-cross
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
	DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	cargo zigbuild --release --target x86_64-pc-windows-gnu --no-default-features -p cowen-cli -p cowen-daemon
	cp target/x86_64-pc-windows-gnu/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x86_64-cross/$(BINARY).exe
	cp target/x86_64-pc-windows-gnu/release/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86_64-cross/cowen-daemon.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64-cross/$(BINARY).exe)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64-cross/$(BINARY).exe)

windows-plugin-x64:
	@echo "🪟 Building Windows x86_64 AI plugin (MSVC Release)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "cargo build --release --target x86_64-pc-windows-msvc -p cowen-search-embedding"
	powershell -NoProfile -Command "cargo run --release -p cowen-signer -- sign-plugin --dylib target/x86_64-pc-windows-msvc/release/cowen_search_embedding.dll --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-pc-windows-msvc/release/cowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/cowen_search_embedding.dll -Destination $(OUTPUT_DIR)/windows-x86_64/cowen_search_embedding.dll -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/cowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/cowen_search_embedding.bundle -Force"

windows-plugin-x86:
	@echo "🪟 Building Windows x86 (32-bit) AI plugin (MSVC Release)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86"
	powershell -NoProfile -Command "cargo build --release --target i686-pc-windows-msvc -p cowen-search-embedding"
	powershell -NoProfile -Command "cargo run --release -p cowen-signer -- sign-plugin --dylib target/i686-pc-windows-msvc/release/cowen_search_embedding.dll --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/i686-pc-windows-msvc/release/cowen_search_embedding.bundle --capabilities SearchProvider --required-privileges LocalCacheAccess,ModelAssetFetch,ComputeHeavy"
	powershell -NoProfile -Command "Copy-Item -Path target/i686-pc-windows-msvc/release/cowen_search_embedding.dll -Destination $(OUTPUT_DIR)/windows-x86/cowen_search_embedding.dll -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/i686-pc-windows-msvc/release/cowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86/cowen_search_embedding.bundle -Force"
# --- 打包目标 ---

package-macos-aarch64: clean macos-aarch64
	@echo "📦 Packaging macOS Apple Silicon (Component Based)..."
	@$(MAKE) impl-package-macos ARCH=aarch64

impl-package-macos:
	@echo "📦 Building Component Packages for $(ARCH)..."
	mkdir -p pkg_core_root/usr/local/bin pkg_core_root/usr/local/share/doc/cowen $(OUTPUT_DIR)/macos-$(ARCH)/release
	mkdir -p pkg_plugin_root/usr/local/share/cowen/staging
	cp $(OUTPUT_DIR)/macos-$(ARCH)/$(BINARY) pkg_core_root/usr/local/bin/$(BINARY)
	cp $(OUTPUT_DIR)/macos-$(ARCH)/cowen-daemon pkg_core_root/usr/local/bin/cowen-daemon
	cp dist_assets/macos/cowen-uninstall pkg_core_root/usr/local/bin/cowen-uninstall
	cp CHANGELOG.md pkg_core_root/usr/local/share/doc/cowen/
	cp -r dist_assets/usage pkg_core_root/usr/local/share/doc/cowen/usage
	cp target/release/libcowen_search_embedding pkg_plugin_root/usr/local/share/cowen/staging/ || true
	if [ -f "pkg_plugin_root/usr/local/share/cowen/staging/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle pkg_plugin_root/usr/local/share/cowen/staging/ || exit 1; fi
	cp target/release/cowen-mcp-plugin pkg_plugin_root/usr/local/share/cowen/staging/ || true
	if [ -f "pkg_plugin_root/usr/local/share/cowen/staging/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle pkg_plugin_root/usr/local/share/cowen/staging/ || exit 1; fi
	@chmod +x dist_assets/macos/scripts_core/postinstall
	@chmod +x dist_assets/macos/scripts_plugin/postinstall
	pkgbuild --identifier com.chanjet.$(BINARY).core.$(ARCH) --version $(VERSION) --root pkg_core_root --install-location / --scripts dist_assets/macos/scripts_core $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-core.pkg
	pkgbuild --identifier com.chanjet.$(BINARY).plugin.$(ARCH) --version $(VERSION) --root pkg_plugin_root --install-location / --scripts dist_assets/macos/scripts_plugin $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin.pkg
	@echo "🧩 Synthesizing Distribution UI..."
	cd $(OUTPUT_DIR)/macos-$(ARCH)/release && \
	productbuild --synthesize --package cowen-core.pkg --package cowen-plugin.pkg Distribution.xml && \
	sed -i '' 's/customize="never"/customize="always"/' Distribution.xml && \
	perl -0777 -pi -e 's#<choices-outline>.*?</choices-outline>#<choices-outline>\\n        <line choice="com.chanjet.$(BINARY).core.$(ARCH)"/>\\n        <line choice="com.chanjet.$(BINARY).plugin.$(ARCH)"/>\\n    </choices-outline>#s' Distribution.xml && \
	perl -pi -e 's#<choice id="default"/>##g' Distribution.xml && \
	sed -i '' 's/<choice id="com.chanjet.$(BINARY).core.$(ARCH)".*>/<choice id="com.chanjet.$(BINARY).core.$(ARCH)" title="Cowen Core CLI" description="The core Cowen CLI and daemon." visible="true" enabled="false" selected="true">/' Distribution.xml && \
	sed -i '' 's/<choice id="com.chanjet.$(BINARY).plugin.$(ARCH)".*>/<choice id="com.chanjet.$(BINARY).plugin.$(ARCH)" title="Cowen Plugins" description="Installs the bundled plugins (AI Embedding, MCP Plugin)." visible="true" selected="true">/' Distribution.xml && \
	productbuild --distribution Distribution.xml --package-path . $(BINARY)-v$(VERSION)-macos-$(ARCH).pkg
	rm -rf pkg_core_root pkg_plugin_root
	rm -f $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-core.pkg $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin.pkg $(OUTPUT_DIR)/macos-$(ARCH)/release/Distribution.xml
	cp dist_assets/macos/cowen-uninstall $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-uninstall
	@$(call MD5,$(OUTPUT_DIR)/macos-$(ARCH)/release/$(BINARY)-v$(VERSION)-macos-$(ARCH).pkg)
	@$(call SHA1,$(OUTPUT_DIR)/macos-$(ARCH)/release/$(BINARY)-v$(VERSION)-macos-$(ARCH).pkg)

package-linux-x86_64: clean linux-x86_64
	@echo "📦 Packaging Linux x86_64 (Native Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-x86_64-with-podman: clean linux-x86_64-with-podman
	@echo "📦 Packaging Linux x86_64 (Podman Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-aarch64: clean linux-aarch64
	@echo "📦 Packaging Linux aarch64..."
	mkdir -p $(OUTPUT_DIR)/linux-aarch64/release pkg_root_aarch64/lib
	cp target/release/libcowen_search_embedding pkg_root_aarch64/lib/ || true
	if [ -f "pkg_root_aarch64/lib/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle pkg_root_aarch64/lib/ || exit 1; fi
	cp target/release/cowen-mcp-plugin pkg_root_aarch64/lib/ || true
	if [ -f "pkg_root_aarch64/lib/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle pkg_root_aarch64/lib/ || exit 1; fi
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-aarch64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-aarch64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-aarch64/
	cp -r dist_assets/usage $(OUTPUT_DIR)/linux-aarch64/
	cd $(OUTPUT_DIR)/linux-aarch64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz $(BINARY) cowen-daemon install.sh README.txt CHANGELOG.md usage -C ../../cli/cowen/pkg_root_aarch64 lib
	rm -rf pkg_root_aarch64
	@$(call MD5,$(OUTPUT_DIR)/linux-aarch64/release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz)
	@$(call SHA1,$(OUTPUT_DIR)/linux-aarch64/release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz)

package-windows-x86_64: clean windows-x86_64
	@echo "📦 Packaging Windows x86_64..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64/release"
	powershell -NoProfile -Command "Copy-Item -Path CHANGELOG.md -Destination ../cowen_setup/ -Force"
	powershell -NoProfile -Command "cd ../cowen_setup; $$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "Copy-Item -Path ../cowen_setup/target/x86_64-pc-windows-msvc/release/cowen_setup.exe -Destination $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe -Force"
	@echo "🗜️ Zipping Windows package..."
	powershell -NoProfile -Command "Compress-Archive -Path $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe -DestinationPath $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip -Force"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@echo "🔍 Automating Windows package verification..."
	powershell -NoProfile -ExecutionPolicy Bypass -File "crates/cowen-cli/tests/runners/verify_windows_pkg.ps1" -SetupExePath "$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe"

package-windows-x86_64-cross: windows-x86_64-cross
	@echo "📦 Cross-packaging Windows x86_64 on macOS/Linux..."
	mkdir -p $(OUTPUT_DIR)/windows-x86_64-cross/release
	rm -rf $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg
	mkdir -p $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg
	cp $(OUTPUT_DIR)/windows-x86_64-cross/$(BINARY).exe $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/
	cp $(OUTPUT_DIR)/windows-x86_64-cross/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/
	cp CHANGELOG.md $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/README.txt
	cp ./dist_assets/windows/install.ps1 $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/
	cp -r dist_assets/usage $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/
	@if [ -f "../../assets/plugins/windows-x86_64/cowen_search_embedding.dll" ]; then \
		echo "✅ Found stable cowen_search_embedding.dll in assets, copying into package..."; \
		cp ../../assets/plugins/windows-x86_64/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/;
		cp ../../assets/plugins/windows-x86_64/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/ || true; \
	elif [ -f "$(OUTPUT_DIR)/windows-x86_64-cross/cowen_search_embedding.dll" ]; then \
		echo "✅ Found cowen_search_embedding.dll in output dir (copied from Windows), copying into package..."; \
		cp $(OUTPUT_DIR)/windows-x86_64-cross/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/;
		cp $(OUTPUT_DIR)/windows-x86_64-cross/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/ || true; \
	elif [ -f "target/x86_64-pc-windows-msvc/release/cowen_search_embedding.dll" ]; then \
		echo "✅ Found cowen_search_embedding.dll in target, copying into package..."; \
		cp target/x86_64-pc-windows-msvc/release/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/;
		cp target/x86_64-pc-windows-msvc/release/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/ || true; \
	else \
		echo "⚠️ WARNING: cowen_search_embedding.dll not found. The package will be compiled WITHOUT AI embedding features."; \
	fi
	@if [ -f "$(OUTPUT_DIR)/windows-x86_64-cross/cowen-mcp-plugin.exe" ]; then \
		echo "✅ Found cowen-mcp-plugin.exe in output dir (copied from cross compilation), copying into package..."; \
		cp $(OUTPUT_DIR)/windows-x86_64-cross/cowen-mcp-plugin.exe $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/; \
		cp $(OUTPUT_DIR)/windows-x86_64-cross/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/ || true; \
	elif [ -f "target/x86_64-pc-windows-msvc/release/cowen-mcp-plugin.exe" ]; then \
		echo "✅ Found cowen-mcp-plugin.exe in target, copying into package..."; \
		cp target/x86_64-pc-windows-msvc/release/cowen-mcp-plugin.exe $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/; \
		cp target/x86_64-pc-windows-msvc/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg/ || true; \
	fi
	@echo "🗜️ Zipping Windows package..."
	cd $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg && zip -r ../release/$(BINARY)-v$(VERSION)-windows-x86_64.zip .
	rm -rf $(OUTPUT_DIR)/windows-x86_64-cross/tmp_pkg
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64-cross/release/$(BINARY)-v$(VERSION)-windows-x86_64.zip)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64-cross/release/$(BINARY)-v$(VERSION)-windows-x86_64.zip)
	@echo "✅ Cross-packaging completed: $(OUTPUT_DIR)/windows-x86_64-cross/release/$(BINARY)-v$(VERSION)-windows-x86_64.zip"

package-windows-x86-cross: windows-x86-cross
	@echo "📦 Cross-packaging Windows x86 (32-bit) on macOS/Linux..."
	mkdir -p $(OUTPUT_DIR)/windows-x86/release
	rm -rf $(OUTPUT_DIR)/windows-x86/tmp_pkg
	mkdir -p $(OUTPUT_DIR)/windows-x86/tmp_pkg
	cp $(OUTPUT_DIR)/windows-x86/$(BINARY).exe $(OUTPUT_DIR)/windows-x86/tmp_pkg/
	cp $(OUTPUT_DIR)/windows-x86/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86/tmp_pkg/
	cp CHANGELOG.md $(OUTPUT_DIR)/windows-x86/tmp_pkg/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/windows-x86/tmp_pkg/README.txt
	cp ./dist_assets/windows/install.ps1 $(OUTPUT_DIR)/windows-x86/tmp_pkg/
	@if [ -f "../../assets/plugins/windows-x86/cowen_search_embedding.dll" ]; then \
		echo "✅ Found stable 32-bit cowen_search_embedding.dll in assets, copying into package..."; \
		cp ../../assets/plugins/windows-x86/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86/tmp_pkg/;
		cp ../../assets/plugins/windows-x86/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86/tmp_pkg/ || true; \
	elif [ -f "$(OUTPUT_DIR)/windows-x86/cowen_search_embedding.dll" ]; then \
		echo "✅ Found 32-bit cowen_search_embedding.dll in output dir (copied from Windows), copying into package..."; \
		cp $(OUTPUT_DIR)/windows-x86/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86/tmp_pkg/;
		cp $(OUTPUT_DIR)/windows-x86/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86/tmp_pkg/ || true; \
	elif [ -f "target/i686-pc-windows-msvc/release/cowen_search_embedding.dll" ]; then \
		echo "✅ Found 32-bit cowen_search_embedding.dll in target, copying into package..."; \
		cp target/i686-pc-windows-msvc/release/cowen_search_embedding.dll $(OUTPUT_DIR)/windows-x86/tmp_pkg/;
		cp target/i686-pc-windows-msvc/release/cowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86/tmp_pkg/ || true; \
	else \
		echo "⚠️ WARNING: 32-bit cowen_search_embedding.dll not found. The package will be compiled WITHOUT AI embedding features."; \
	fi
	@if [ -f "$(OUTPUT_DIR)/windows-x86/cowen-mcp-plugin.exe" ]; then \
		echo "✅ Found 32-bit cowen-mcp-plugin.exe in output dir, copying into package..."; \
		cp $(OUTPUT_DIR)/windows-x86/cowen-mcp-plugin.exe $(OUTPUT_DIR)/windows-x86/tmp_pkg/; \
		cp $(OUTPUT_DIR)/windows-x86/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/windows-x86/tmp_pkg/ || true; \
	elif [ -f "target/i686-pc-windows-msvc/release/cowen-mcp-plugin.exe" ]; then \
		echo "✅ Found 32-bit cowen-mcp-plugin.exe in target, copying into package..."; \
		cp target/i686-pc-windows-msvc/release/cowen-mcp-plugin.exe $(OUTPUT_DIR)/windows-x86/tmp_pkg/; \
		cp target/i686-pc-windows-msvc/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/windows-x86/tmp_pkg/ || true; \
	fi
	@echo "🗜️ Zipping Windows package..."
	cd $(OUTPUT_DIR)/windows-x86/tmp_pkg && zip -r ../release/$(BINARY)-v$(VERSION)-windows-x86.zip .
	rm -rf $(OUTPUT_DIR)/windows-x86/tmp_pkg
	@$(call MD5,$(OUTPUT_DIR)/windows-x86/release/$(BINARY)-v$(VERSION)-windows-x86.zip)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86/release/$(BINARY)-v$(VERSION)-windows-x86.zip)
	@echo "✅ Cross-packaging completed: $(OUTPUT_DIR)/windows-x86/release/$(BINARY)-v$(VERSION)-windows-x86.zip"

# 内部复用的 Linux 打包逻辑
impl-package-linux-x86_64:
	mkdir -p $(OUTPUT_DIR)/linux-x86_64/release pkg_root/lib
	cp $(OUTPUT_DIR)/linux-x86_64/libcowen_search_embedding pkg_root/lib/ || true
	if [ -f "pkg_root/lib/libcowen_search_embedding" ]; then cp $(OUTPUT_DIR)/linux-x86_64/libcowen_search_embedding.bundle pkg_root/lib/ || exit 1; fi
	cp $(OUTPUT_DIR)/linux-x86_64/cowen-mcp-plugin pkg_root/lib/ || true
	if [ -f "pkg_root/lib/cowen-mcp-plugin" ]; then cp $(OUTPUT_DIR)/linux-x86_64/cowen-mcp-plugin.bundle pkg_root/lib/ || exit 1; fi
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x86_64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x86_64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-x86_64/
	cp -r dist_assets/usage $(OUTPUT_DIR)/linux-x86_64/
	cd $(OUTPUT_DIR)/linux-x86_64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz $(BINARY) cowen-daemon install.sh README.txt CHANGELOG.md usage -C ../../cli/cowen/pkg_root lib
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)
	@$(call SHA1,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)

# --- 测试目标 (Test Targets) ---

test-rust:
	@echo "🧪 Running Rust internal and integration tests..."
	RUSTFLAGS="-D warnings" cargo build -p cowen-cli -p cowen-daemon
	COWEN_SKIP_BROWSER=true RUSTFLAGS="-D warnings" cargo test

# 自动根据环境运行测试
test:
ifeq ($(HOST_OS),macos)
	@$(MAKE) test-macos
else ifeq ($(HOST_OS),linux)
	@$(MAKE) test-linux
else ifeq ($(HOST_OS),windows)
	@$(MAKE) test-win
endif

# 在 Linux 容器中执行并行 E2E (Podman) - 封闭纯净环境
# 在 Linux 容器中执行并行 E2E (Podman) - All-in-One 封闭容器模式
# 数据库服务内置于测试容器中，无需外部 db-up，无需 --network host
test-linux: prepare-podman-machine prepare-podman-image
	@echo "🧪 Running Full Test Suite in Linux container (All-in-One mode)..."
	@export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || echo "unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"); \
	trap '$(MAKE) stop-podman-machine' EXIT; \
	$(PODMAN_RUN_BASE) \
		-it \
		-e DB_HOST=127.0.0.1 \
		-e MAX_PARALLEL=$(MAX_PARALLEL) \
		-e QEMU_LD_PREFIX=/usr/x86_64-linux-gnu/ \
		$(BUILDER_IMAGE) \
		bash -c "COWEN_SKIP_BROWSER=true RUSTFLAGS=\"-D warnings\" cargo test --release -j $(MAX_PARALLEL) --target x86_64-unknown-linux-gnu && COWEN_SKIP_BROWSER=true crates/cowen-cli/tests/runners/run_parallel.sh"

# 在 Windows PowerShell 环境下执行测试

# 在 Windows PowerShell 环境下执行测试
test-win: test-rust
	@echo "🧪 Running Full E2E Suite in Windows PowerShell..."
	powershell -NoProfile -ExecutionPolicy Bypass -Command "$$env:COWEN_SKIP_BROWSER='true'; crates/cowen-cli/tests/runners/run_suites.ps1"

# 在 macOS 原生环境执行并行测试
test-macos: test-rust local-db-up
	@echo "🧪 Running Parallel E2E Suite natively on macOS (Isolated)..."
	COWEN_SKIP_BROWSER=true TEST_BASE=target/cowen_tests_macos BASE_PORT_START=18000 crates/cowen-cli/tests/runners/run_parallel.sh

# --- 数据库管理 (Database Management) ---

# 探测 Compose 工具
COMPOSE_BIN := $(shell command -v docker-compose 2> /dev/null || command -v podman-compose 2> /dev/null)

# --- Homebrew 本地数据库支持 (Homebrew Local DB Support) ---
brew-deps-install:
	@echo "🍺 Installing MySQL and PostgreSQL via Homebrew..."
	brew install mysql postgresql@15
	@echo "✅ Dependencies installed."

local-db-up:
	@echo "🚀 Starting local MySQL and PostgreSQL services..."
	brew services start mysql || brew services restart mysql
	brew services start postgresql@15 || brew services restart postgresql@15
	@echo "⌛ Waiting for local services to be ready..."
	@sleep 5
	@mysql -u root -h 127.0.0.1 -e "CREATE DATABASE IF NOT EXISTS cowen_test;" < /dev/null || echo "⚠️ MySQL DB create failed, might already exist"
	@psql -h 127.0.0.1 -w -d postgres -c "CREATE DATABASE cowen_test;" || echo "⚠️ Postgres DB create failed, might already exist"
	@echo "✅ Local databases are ready."

local-db-down:
	@echo "🛑 Stopping local MySQL and PostgreSQL services..."
	brew services stop mysql || true
	brew services stop postgresql@15 || true

# 专用于 Podman 测试的统一网络 Pod 模式
db-up-pod: prepare-podman-machine
	@echo "🚀 Aggressively purging all Cowen test resources..."
	@export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || echo "unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"); \
	podman stop -a 2>/dev/null || true; \
	podman rm -f -a 2>/dev/null || true; \
	podman pod rm -f -a 2>/dev/null || true; \
	sleep 2; \
	echo "🚀 Starting MySQL, PostgreSQL and Redis in a unified Podman Pod..."; \
	podman pod create --name cowen-test-pod -p 3306:3306 -p 5432:5432 -p 6379:6379 || exit 1; \
	podman run -d --pod cowen-test-pod --name cowen-mysql -e MYSQL_ROOT_PASSWORD=root -e MYSQL_DATABASE=cowen_test --health-cmd="mysqladmin ping -h localhost" --health-interval=2s mysql:8.0 || exit 1; \
	podman run -d --pod cowen-test-pod --name cowen-postgres -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=password -e POSTGRES_DB=cowen_test --health-cmd="pg_isready -U postgres" --health-interval=2s postgres:15 || exit 1; \
	podman run -d --pod cowen-test-pod --name cowen-redis --health-cmd="redis-cli ping" --health-interval=2s redis:7.0-alpine || exit 1; \
	echo "⌛ Waiting for databases in Pod to be ready..."; \
	MAX_WAIT=20; count=0; \
	while [ $$count -lt $$MAX_WAIT ]; do \
		HEALTHY_COUNT=$$(podman ps --filter "pod=cowen-test-pod" --filter "health=healthy" --format "{{.ID}}" | wc -l); \
		if [ $$HEALTHY_COUNT -ge 3 ]; then \
			echo -e "\n✅ Databases in Pod are ready."; \
			exit 0; \
		fi; \
		printf "."; sleep 2; count=$$((count + 1)); \
	done; \
	echo -e "\n❌ TIMEOUT: Databases failed to become healthy."; \
	podman ps -a --filter "pod=cowen-test-pod"; \
	exit 1

db-down-pod:
	@echo "🛑 Stopping and removing Podman Pod..."
	@export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || echo "unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"); \
	podman stop -a 2>/dev/null || true; \
	podman rm -f -a 2>/dev/null || true; \
	podman pod rm -f -a 2>/dev/null || true; \
	$(MAKE) stop-podman-machine

db-up: prepare-podman-machine
	@echo "🚀 Starting MySQL, PostgreSQL and Redis containers..."
	@export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || echo "unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"); \
	if [ "$(COMPOSE_BIN)" = "" ]; then \
		echo "⚠️  No compose provider found, falling back to 'podman run'..."; \
		podman stop -a 2>/dev/null || true; \
		podman rm -f -a 2>/dev/null || true; \
		podman run -d --name cowen-mysql -p 3306:3306 -e MYSQL_ROOT_PASSWORD=root -e MYSQL_DATABASE=cowen_test --health-cmd="mysqladmin ping -h localhost" --health-interval=5s mysql:8.0; \
		podman run -d --name cowen-postgres -p 5432:5432 -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=password -e POSTGRES_DB=cowen_test --health-cmd="pg_isready -U postgres" --health-interval=5s postgres:15; \
		podman run -d --name cowen-redis -p 6379:6379 --health-cmd="redis-cli ping" --health-interval=5s redis:7.0-alpine; \
	else \
		echo "🚀 Using $(COMPOSE_BIN)..."; \
		cd crates/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v 2>/dev/null || true; \
		cd crates/cowen-cli/tests/infra && $(COMPOSE_BIN) up -d || (echo "❌ Failed to start containers." && exit 1); \
	fi; \
	echo "⌛ Waiting for databases to be ready..."; \
	MAX_WAIT=30; count=0; \
	until [ $$count -ge $$MAX_WAIT ] || [ $$(podman ps 2>/dev/null | grep 'healthy' | wc -l) -ge 3 ]; do \
		echo -n "."; sleep 2; count=$$((count + 1)); \
	done; \
	echo " ✅ Databases are ready."

db-down:
	@export DOCKER_HOST=$$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || echo "unix://$$HOME/.local/share/containers/podman/machine/podman-machine-default/podman.sock"); \
	echo "🛑 Stopping and removing containers..."; \
	if [ "$(COMPOSE_BIN)" = "" ]; then \
		podman stop cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
		podman rm cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	else \
		cd crates/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v; \
	fi; \
	$(MAKE) stop-podman-machine

clean:
	cargo clean
	$(call RM,$(OUTPUT_DIR)/macos-aarch64 $(OUTPUT_DIR)/linux-x86_64 $(OUTPUT_DIR)/linux-aarch64 $(OUTPUT_DIR)/windows-x86 $(OUTPUT_DIR)/windows-x86_64 $(OUTPUT_DIR)/windows-x86_64-cross $(OUTPUT_DIR)/test .builder_image_built target/cowen_tests_linux target/cowen_tests_macos target/cowen_tests)

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

install: build
	@echo "🚀 Installing core binaries to $(BINDIR)/..."
	mkdir -p $(BINDIR)
	rm -f $(BINDIR)/$(BINARY) $(BINDIR)/cowen-daemon
	cp target/release/$(BINARY) $(BINDIR)/$(BINARY)
	cp target/release/cowen-daemon $(BINDIR)/cowen-daemon
	@echo "🚀 Installing plugins to $(HOME)/.cowen/plugins/..."
	@mkdir -p $(HOME)/.cowen/plugins
	@rm -f $(HOME)/.cowen/plugins/cowen-mcp-plugin $(HOME)/.cowen/plugins/cowen-mcp-plugin.bundle
	@rm -f $(HOME)/.cowen/plugins/libcowen_search_embedding $(HOME)/.cowen/plugins/libcowen_search_embedding.bundle
	cp target/release/cowen-mcp-plugin $(HOME)/.cowen/plugins/ || true
	cp target/release/cowen-mcp-plugin.bundle $(HOME)/.cowen/plugins/ || true
	cp target/release/libcowen_search_embedding $(HOME)/.cowen/plugins/ || true
	cp target/release/libcowen_search_embedding.bundle $(HOME)/.cowen/plugins/ || true
	@echo "🚀 Installed to $(BINDIR)/$(BINARY), $(BINDIR)/cowen-daemon and $(HOME)/.cowen/plugins/"

run:
	cargo run -- --help

# Makefile 跨平台编译语法检查强校准靶点
check-cross:
	@echo "====== 开始 macOS 架构静态编译检查 ======"
	cargo check --target aarch64-apple-darwin --workspace --all-targets
	@echo "====== 开始 Linux 架构静态编译检查 ======"
	cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets
	@echo "====== 开始 Windows 架构静态编译检查 ======"
	cargo check --target x86_64-pc-windows-msvc --workspace --all-targets
	@echo "====== 所有平台跨平台编译静态检查 100% 通过 ======"

