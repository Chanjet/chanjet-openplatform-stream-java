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
BUILDER_IMAGE=cowen-builder

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

VERIFY_BIN = ./tests/verify-binary.sh $(1)

# 生产/测试环境配置
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build macos-aarch64 macos-x86_64 linux-x86_64 linux-x86_64-with-podman linux-aarch64 windows-x86_64 test-macos-aarch64 \
        test test-linux test-win test-macos \
        package-macos-aarch64 package-macos-x86_64 package-linux-x86_64 package-linux-x86_64-with-podman \
        package-linux-aarch64 package-windows-x86_64 clean install run explore prepare-podman-image prepare-podman-machine stop-podman-machine

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

# 确保 Podman 虚拟机已启动 (仅 macOS/Windows)
prepare-podman-machine:
ifeq ($(HOST_OS),macos)
	@if podman machine list --format '{{.Default}} {{.Running}}' 2>/dev/null | grep -q '^true false'; then \
		echo "🚀 Starting Podman machine..."; \
		podman machine start; \
	fi
endif

# 停止 Podman 虚拟机 (仅 macOS)
stop-podman-machine:
ifeq ($(HOST_OS),macos)
	@echo "🛑 Stopping Podman machine to recycle resources..."
	@podman machine stop 2>/dev/null || true
endif

# 准备 Podman 编译镜像 (固化系统依赖)
prepare-podman-image: prepare-podman-machine
	@if ! podman image exists $(BUILDER_IMAGE); then \
		echo "🛠️ Creating custom builder image $(BUILDER_IMAGE)..."; \
		printf "FROM docker.io/library/rust:latest\nRUN apt-get update && apt-get install -y g++-x86-64-linux-gnu libc6-amd64-cross && rustup target add x86_64-unknown-linux-gnu" | podman build -t $(BUILDER_IMAGE) -; \
	fi

# 提速版 Podman 构建 (系统依赖 + Rust 依赖双缓存)
linux-x86_64-with-podman: prepare-podman-image
	@echo "🐧 Building Linux x86_64 [FULL VERSION] via Podman v$(VERSION)..."
	@mkdir -p $(OUTPUT_DIR)/linux-x86_64
	@mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git
	@trap '$(MAKE) stop-podman-machine' EXIT; \
	podman run --rm \
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
	powershell -NoProfile -ExecutionPolicy Bypass -File "./tests/verify_windows_pkg.ps1" -SetupExePath "$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe"

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

# --- 测试目标 (Test Targets) ---

# 自动根据环境运行测试
test:
ifeq ($(HOST_OS),macos)
	@$(MAKE) test-macos
else ifeq ($(HOST_OS),linux)
	@$(MAKE) test-linux
else ifeq ($(HOST_OS),windows)
	@$(MAKE) test-win
endif

# 在 Linux 容器中执行全量 E2E (Podman) - 支持缓存共享
test-linux: prepare-podman-machine prepare-podman-image
	@echo "🧪 Running Full E2E Suite in Linux container with Shared Cache..."
	@if ! podman image exists cowen-test-base; then \
		podman build -t cowen-test-base -f tests/Dockerfile.linux.base .; \
	fi
	podman run --rm -it \
		-v "$(shell pwd)/../..:/workspace" \
		-v "$(HOME)/.cargo/registry:/usr/local/cargo/registry" \
		-v "$(HOME)/.cargo/git:/usr/local/cargo/git" \
		-w /workspace/cli/cowen \
		cowen-test-base \
		./tests/run_suites.sh

# 在 Windows PowerShell 环境下执行测试
test-win:
	@echo "🧪 Running Full E2E Suite in Windows PowerShell..."
	powershell -NoProfile -ExecutionPolicy Bypass -File "./tests/run_suites.ps1"

# 在 macOS 原生环境执行测试
test-macos:
	@echo "🧪 Running Full E2E Suite natively on macOS..."
	./tests/run_suites.sh

# --- 其他 ---

clean:
	cargo clean
	$(call RM,$(OUTPUT_DIR)/macos-aarch64 $(OUTPUT_DIR)/macos-x86_64 $(OUTPUT_DIR)/linux-x86_64 $(OUTPUT_DIR)/linux-aarch64 $(OUTPUT_DIR)/windows-x86_64 $(OUTPUT_DIR)/test)

install:
	@$(MAKE) macos-aarch64
	cp $(OUTPUT_DIR)/macos-aarch64/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"

run:
	cargo run -- --help

explore:
	@echo "🔍 Running exploratory ready tests for $(HOST_OS)-$(HOST_ARCH)..."
	@if [ "$(HOST_OS)" = "windows" ]; then \
		./tests/exploratory_ready_test.sh $(OUTPUT_DIR)/$(HOST_OS)-$(HOST_ARCH)/$(BINARY).exe; \
	else \
		./tests/exploratory_ready_test.sh $(OUTPUT_DIR)/$(HOST_OS)-$(HOST_ARCH)/$(BINARY); \
	fi

