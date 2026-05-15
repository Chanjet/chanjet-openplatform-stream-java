BINARY=cowen
PKG=cli/cowen
OUTPUT_DIR=../../bin

# 从 Cargo.toml 自动提取版本号
ifeq ($(OS),Windows_NT)
    VERSION := $(shell powershell -NoProfile -Command "(Get-Content Cargo.toml | Select-String '^version =').ToString().Split('\"')[1].Trim()")
else
    VERSION := $(shell grep '^version =' Cargo.toml | cut -d '"' -f 2)
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

VERIFY_BIN = ./tests/e2e/scripts/verify-binary.sh $(1)

# 生产/测试环境配置
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build macos-aarch64 macos-x86_64 linux-x86_64 linux-x86_64-with-podman linux-x86_64-legacy-with-podman linux-aarch64 windows-x86_64 windows-x86_64-legacy test-macos-aarch64 \
        test test-linux test-win test-macos \
        package-macos-aarch64 package-macos-x86_64 package-linux-x86_64 package-linux-x86_64-with-podman package-linux-x86_64-legacy-with-podman \
        package-linux-aarch64 package-windows-x86_64 package-windows-x86_64-legacy clean install run prepare-podman-image prepare-podman-machine stop-podman-machine

# --- 默认构建目标 (Default Build Target) ---
build:
ifeq ($(HOST_OS),macos)
ifeq ($(HOST_ARCH),aarch64)
	@$(MAKE) macos-aarch64
else
	@$(MAKE) macos-x86_64
endif
else ifeq ($(HOST_OS),linux)
ifeq ($(HOST_ARCH),x86_64)
	@$(MAKE) linux-x86_64
else
	@$(MAKE) linux-aarch64
endif
else ifeq ($(HOST_OS),windows)
	@$(MAKE) windows-x86_64
endif

# 授权配置 (Authentication)
OFFICIAL_APP_KEY = 3NWdEbmu    # 正式环境 AppKey (Production)
PREVIEW_APP_KEY  ?= RjP3LPoI   # 预览/集成测试环境 AppKey (Preview/INTE)

# --- 核心构建目标 ---

macos-aarch64:
	@echo "🍎 Building macOS Apple Silicon v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/macos-aarch64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo build --release
	cp target/release/$(BINARY) $(OUTPUT_DIR)/macos-aarch64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-aarch64/$(BINARY) 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))

test-macos-aarch64:
	@echo "🧪 Building macOS Apple Silicon (TEST ENV) v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/test
	CARGO_BIN_NAME_OVERRIDE=$(BINARY)-test APP_DIR_NAME=.$(BINARY)-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) BUILTIN_CLIENT_ID=$(PREVIEW_APP_KEY) cargo build --release --features inte
	cp target/release/$(BINARY) $(OUTPUT_DIR)/test/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/test/$(BINARY) 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/test/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/test/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/test/$(BINARY))

macos-x86_64:
	@echo "🍎 Building macOS Intel v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/macos-x86_64
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo build --release --target x86_64-apple-darwin --no-default-features
	cp target/x86_64-apple-darwin/release/$(BINARY) $(OUTPUT_DIR)/macos-x86_64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-x86_64/$(BINARY) 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/macos-x86_64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/macos-x86_64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/macos-x86_64/$(BINARY))

linux-x86_64:
	@echo "🐧 Building Linux x86_64 [FULL VERSION] natively v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-x86_64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(OUTPUT_DIR)/linux-x86_64/$(BINARY)
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

# 提速版 Podman 构建 (系统依赖 + Rust 依赖双缓存)
linux-x86_64-with-podman: prepare-podman-image
	@echo "🐧 Building Linux x86_64 [FULL VERSION] via Podman v$(VERSION)..."
	@mkdir -p $(OUTPUT_DIR)/linux-x86_64
	@mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git
	@trap '$(MAKE) stop-podman-machine' EXIT; \
	podman run --rm --platform linux/amd64 \
		-v "$$(pwd)/../..:/workspace" \
		-v "$(CARGO_CACHE_DIR)/registry:/usr/local/cargo/registry" \
		-v "$(CARGO_CACHE_DIR)/git:/usr/local/cargo/git" \
		-w /workspace/cli/cowen \
		$(BUILDER_IMAGE) \
		bash -c "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc && \
			CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
			DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
			BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
			cargo build --release --target x86_64-unknown-linux-gnu && \
			cp target/x86_64-unknown-linux-gnu/release/$(BINARY) ../../bin/linux-x86_64/$(BINARY)" && \
	$(call MD5,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call SHA1,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))

# 提速版 Podman 构建 (Legacy/No-AVX)
linux-x86_64-legacy-with-podman: prepare-podman-image
	@echo "🐧 Building Linux x86_64 (Legacy/No-AVX) via Podman v$(VERSION)..."
	@mkdir -p $(OUTPUT_DIR)/linux-x86_64-legacy
	@mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git
	@trap '$(MAKE) stop-podman-machine' EXIT; \
	podman run --rm --platform linux/amd64 \
		-v "$$(pwd)/../..:/workspace" \
		-v "$(CARGO_CACHE_DIR)/registry:/usr/local/cargo/registry" \
		-v "$(CARGO_CACHE_DIR)/git:/usr/local/cargo/git" \
		-w /workspace/cli/cowen \
		$(BUILDER_IMAGE) \
		bash -c "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc && \
			CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
			DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
			BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
			cargo build --release --target x86_64-unknown-linux-gnu --no-default-features && \
			cp target/x86_64-unknown-linux-gnu/release/$(BINARY) ../../bin/linux-x86_64-legacy/$(BINARY)" && \
	$(call MD5,$(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY)) && \
	$(call SHA1,$(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY)) && \
	$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY))

linux-aarch64:
	@echo "🐧 Building Linux aarch64 v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-aarch64
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) cargo zigbuild --release --target aarch64-unknown-linux-musl --no-default-features
	cp target/aarch64-unknown-linux-musl/release/$(BINARY) $(OUTPUT_DIR)/linux-aarch64/$(BINARY)
	@$(call MD5,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))
	@$(call SHA1,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))

windows-x86_64:
	@echo "🪟 Building Windows x86_64 v$(VERSION)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "$$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; $$env:BUILTIN_CLIENT_ID='$(OFFICIAL_APP_KEY)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/$(BINARY).exe -Destination $(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe -Force"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)

windows-x86_64-legacy:
	@echo "🪟 Building Windows x86_64 (Legacy/No-AVX) v$(VERSION)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64-legacy"
	powershell -NoProfile -Command "$$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; $$env:BUILTIN_CLIENT_ID='$(OFFICIAL_APP_KEY)'; cargo build --release --target x86_64-pc-windows-msvc --no-default-features"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/$(BINARY).exe -Destination $(OUTPUT_DIR)/windows-x86_64-legacy/$(BINARY).exe -Force"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64-legacy/$(BINARY).exe)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64-legacy/$(BINARY).exe)

# --- 打包目标 ---

package-macos-aarch64: clean macos-aarch64
	@echo "📦 Packaging macOS Apple Silicon..."
	mkdir -p pkg_root/usr/local/bin pkg_root/usr/local/share/doc/cowen $(OUTPUT_DIR)/macos-aarch64/release
	cp $(OUTPUT_DIR)/macos-aarch64/$(BINARY) pkg_root/usr/local/bin/$(BINARY)
	cp CHANGELOG.md pkg_root/usr/local/share/doc/cowen/
	@chmod +x dist_assets/macos/scripts/postinstall
	pkgbuild --identifier com.chanjet.$(BINARY).aarch64 --version $(VERSION) --root pkg_root --install-location / --scripts dist_assets/macos/scripts $(OUTPUT_DIR)/macos-aarch64/release/$(BINARY)-v$(VERSION)-macos-aarch64.pkg
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/macos-aarch64/release/$(BINARY)-v$(VERSION)-macos-aarch64.pkg)
	@$(call SHA1,$(OUTPUT_DIR)/macos-aarch64/release/$(BINARY)-v$(VERSION)-macos-aarch64.pkg)

package-macos-x86_64: clean macos-x86_64
	@echo "📦 Packaging macOS Intel..."
	mkdir -p pkg_root/usr/local/bin pkg_root/usr/local/share/doc/cowen $(OUTPUT_DIR)/macos-x86_64/release
	cp $(OUTPUT_DIR)/macos-x86_64/$(BINARY) pkg_root/usr/local/bin/$(BINARY)
	cp CHANGELOG.md pkg_root/usr/local/share/doc/cowen/
	@chmod +x dist_assets/macos/scripts/postinstall
	pkgbuild --identifier com.chanjet.$(BINARY).x86_64 --version $(VERSION) --root pkg_root --install-location / --scripts dist_assets/macos/scripts $(OUTPUT_DIR)/macos-x86_64/release/$(BINARY)-v$(VERSION)-macos-x86_64.pkg
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/macos-x86_64/release/$(BINARY)-v$(VERSION)-macos-x86_64.pkg)
	@$(call SHA1,$(OUTPUT_DIR)/macos-x86_64/release/$(BINARY)-v$(VERSION)-macos-x86_64.pkg)

package-linux-x86_64: clean linux-x86_64
	@echo "📦 Packaging Linux x86_64 (Native Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-x86_64-with-podman: clean linux-x86_64-with-podman
	@echo "📦 Packaging Linux x86_64 (Podman Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-x86_64-legacy-with-podman: clean linux-x86_64-legacy-with-podman
	@echo "📦 Packaging Linux x86_64 (Legacy/Podman Build)..."
	@$(MAKE) impl-package-linux-x86_64-legacy

package-linux-aarch64: clean linux-aarch64
	@echo "📦 Packaging Linux aarch64..."
	mkdir -p $(OUTPUT_DIR)/linux-aarch64/release
	cp $(OUTPUT_DIR)/linux-aarch64/$(BINARY) $(OUTPUT_DIR)/linux-aarch64/$(BINARY)-v$(VERSION)-linux-aarch64
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-aarch64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-aarch64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-aarch64/
	cd $(OUTPUT_DIR)/linux-aarch64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz $(BINARY)-v$(VERSION)-linux-aarch64 install.sh README.txt CHANGELOG.md
	rm -f $(OUTPUT_DIR)/linux-aarch64/$(BINARY)-v$(VERSION)-linux-aarch64
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
	powershell -NoProfile -ExecutionPolicy Bypass -File "./tests/runners/verify_windows_pkg.ps1" -SetupExePath "$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe"

package-windows-x86_64-legacy: clean windows-x86_64-legacy
	@echo "📦 Packaging Windows x86_64 (Legacy)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64-legacy/release"
	@echo "🛠️ Preparing legacy binary for setup..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "Copy-Item -Path $(OUTPUT_DIR)/windows-x86_64-legacy/$(BINARY).exe -Destination $(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe -Force"
	powershell -NoProfile -Command "Copy-Item -Path CHANGELOG.md -Destination ../cowen_setup/ -Force"
	powershell -NoProfile -Command "cd ../cowen_setup; $$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "Copy-Item -Path ../cowen_setup/target/x86_64-pc-windows-msvc/release/cowen_setup.exe -Destination $(OUTPUT_DIR)/windows-x86_64-legacy/release/$(BINARY)-v$(VERSION)-windows-x86_64-legacy-setup.exe -Force"
	@echo "🗜️ Zipping Windows package..."
	powershell -NoProfile -Command "Compress-Archive -Path $(OUTPUT_DIR)/windows-x86_64-legacy/release/$(BINARY)-v$(VERSION)-windows-x86_64-legacy-setup.exe -DestinationPath $(OUTPUT_DIR)/windows-x86_64-legacy/release/$(BINARY)-v$(VERSION)-windows-x86_64-legacy-setup.zip -Force"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64-legacy/release/$(BINARY)-v$(VERSION)-windows-x86_64-legacy-setup.zip)
	@$(call SHA1,$(OUTPUT_DIR)/windows-x86_64-legacy/release/$(BINARY)-v$(VERSION)-windows-x86_64-legacy-setup.zip)

# 内部复用的 Linux 打包逻辑
impl-package-linux-x86_64:
	mkdir -p $(OUTPUT_DIR)/linux-x86_64/release
	cp $(OUTPUT_DIR)/linux-x86_64/$(BINARY) $(OUTPUT_DIR)/linux-x86_64/$(BINARY)-v$(VERSION)-linux-x86_64
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x86_64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x86_64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-x86_64/
	cd $(OUTPUT_DIR)/linux-x86_64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz $(BINARY)-v$(VERSION)-linux-x86_64 install.sh README.txt CHANGELOG.md
	rm -f $(OUTPUT_DIR)/linux-x86_64/$(BINARY)-v$(VERSION)-linux-x86_64
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)
	@$(call SHA1,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)

# 内部复用的 Linux 打包逻辑 (Legacy)
impl-package-linux-x86_64-legacy:
	mkdir -p $(OUTPUT_DIR)/linux-x86_64-legacy/release
	cp $(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY) $(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY)-v$(VERSION)-linux-x86_64-legacy
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x86_64-legacy/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x86_64-legacy/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-x86_64-legacy/
	cd $(OUTPUT_DIR)/linux-x86_64-legacy && tar -czf release/$(BINARY)-v$(VERSION)-linux-x86_64-legacy.tar.gz $(BINARY)-v$(VERSION)-linux-x86_64-legacy install.sh README.txt CHANGELOG.md
	rm -f $(OUTPUT_DIR)/linux-x86_64-legacy/$(BINARY)-v$(VERSION)-linux-x86_64-legacy
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64-legacy/release/$(BINARY)-v$(VERSION)-linux-x86_64-legacy.tar.gz)
	@$(call SHA1,$(OUTPUT_DIR)/linux-x86_64-legacy/release/$(BINARY)-v$(VERSION)-linux-x86_64-legacy.tar.gz)

# --- 测试目标 (Test Targets) ---

test-rust:
	@echo "🧪 Running Rust internal and integration tests..."
	cargo test

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
	podman run --rm -it \
		-v $(shell pwd)/../..:/workspace \
		-v $(HOME)/.cargo/registry:/usr/local/cargo/registry \
		-v $(HOME)/.cargo/git:/usr/local/cargo/git \
		-e DB_HOST=127.0.0.1 \
		-e MAX_PARALLEL=$(MAX_PARALLEL) \
		-e QEMU_LD_PREFIX=/usr/x86_64-linux-gnu/ \
		-w /workspace/cli/cowen \
		$(BUILDER_IMAGE) \
		bash -c "cargo test --target x86_64-unknown-linux-gnu && ./tests/runners/run_parallel.sh"

# 在 Windows PowerShell 环境下执行测试
test-win: test-rust
	@echo "🧪 Running Full E2E Suite in Windows PowerShell..."
	powershell -NoProfile -ExecutionPolicy Bypass -File "./tests/runners/run_suites.ps1"

# 在 macOS 原生环境执行并行测试
test-macos: test-rust local-db-up
	@echo "🧪 Running Parallel E2E Suite natively on macOS (Isolated)..."
	TEST_BASE=target/cowen_tests_macos BASE_PORT_START=18000 ./tests/runners/run_parallel.sh

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
		cd tests/infra && $(COMPOSE_BIN) down -v 2>/dev/null || true; \
		cd tests/infra && $(COMPOSE_BIN) up -d || (echo "❌ Failed to start containers." && exit 1); \
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
		cd tests/infra && $(COMPOSE_BIN) down -v; \
	fi; \
	$(MAKE) stop-podman-machine

clean:
	cargo clean
	$(call RM,$(OUTPUT_DIR)/macos-aarch64 $(OUTPUT_DIR)/macos-x86_64 $(OUTPUT_DIR)/linux-x86_64 $(OUTPUT_DIR)/linux-x86_64-legacy $(OUTPUT_DIR)/linux-aarch64 $(OUTPUT_DIR)/windows-x86_64 $(OUTPUT_DIR)/windows-x86_64-legacy $(OUTPUT_DIR)/test .builder_image_built target/cowen_tests_linux target/cowen_tests_macos target/cowen_tests)

install:
	@$(MAKE) macos-aarch64
	cp $(OUTPUT_DIR)/macos-aarch64/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"

run:
	cargo run -- --help

