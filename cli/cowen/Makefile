BINARY=cowen
PKG=cli/cowen
OUTPUT_DIR=../../bin

# 从 Cargo.toml 自动提取版本号
VERSION=$(shell grep '^version =' Cargo.toml | cut -d '"' -f 2)

# 缓存目录与镜像定义
CARGO_CACHE_DIR=$(HOME)/.cache/cowen-cargo
BUILDER_IMAGE=cowen-builder

# 检测当前主机环境
ifeq ($(OS),Windows_NT)
    HOST_OS := windows
    HOST_ARCH := x64
else
    UNAME_S := $(shell uname -s)
    UNAME_M := $(shell uname -m)
    ifeq ($(UNAME_S),Darwin)
        HOST_OS := macos
    else
        HOST_OS := linux
    endif
    ifeq ($(UNAME_M),x86_64)
        HOST_ARCH := x64
    else
        HOST_ARCH := $(UNAME_M)
    endif
endif

# --- 跨平台兼容性宏 ---
ifeq ($(OS),Windows_NT)
    RM = powershell -NoProfile -Command "ForEach($$p in '$(1)'.Split(' ')){ if(Test-Path $$p){ Remove-Item -Recurse -Force $$p } }"
    MD5 = powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm MD5 $(1)).Hash.ToLower(); Set-Content -Path $(1).md5 -Value $$hash"
else
    RM = rm -rf $(1)
    MD5 = (md5 -q $(1) 2>/dev/null || md5sum $(1) | cut -d ' ' -f 1) > $(1).md5
endif

# 生产/测试环境配置
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: macos-arm64 macos-x64 linux-x64 linux-x64-with-podman linux-arm64 windows-x64 test-macos-arm64 \
        package-macos-arm64 package-macos-x64 package-linux-x64 package-linux-x64-with-podman \
        package-linux-arm64 package-windows-x64 clean install run prepare-podman-image

# --- 核心构建目标 ---

macos-arm64:
	@echo "🍎 Building macOS Apple Silicon v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/macos-arm64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) cargo build --release
	cp target/release/$(BINARY) $(OUTPUT_DIR)/macos-arm64/$(BINARY)-v$(VERSION)-macos-arm64
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-arm64/$(BINARY)-v$(VERSION)-macos-arm64 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/macos-arm64/$(BINARY)-v$(VERSION)-macos-arm64)

test-macos-arm64:
	@echo "🧪 Building macOS Apple Silicon (TEST ENV) v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/test
	CARGO_BIN_NAME_OVERRIDE=$(BINARY)-test APP_DIR_NAME=.$(BINARY)-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) cargo build --release
	cp target/release/$(BINARY) $(OUTPUT_DIR)/test/$(BINARY)-v$(VERSION)-test-macos-arm64
	@codesign --force --deep --sign - $(OUTPUT_DIR)/test/$(BINARY)-v$(VERSION)-test-macos-arm64 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/test/$(BINARY)-v$(VERSION)-test-macos-arm64)

macos-x64:
	@echo "🍎 Building macOS Intel v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/macos-x64
	cargo build --release --target x86_64-apple-darwin --no-default-features
	cp target/x86_64-apple-darwin/release/$(BINARY) $(OUTPUT_DIR)/macos-x64/$(BINARY)-v$(VERSION)-macos-x64
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-x64/$(BINARY)-v$(VERSION)-macos-x64 2>/dev/null || true
	@$(call MD5,$(OUTPUT_DIR)/macos-x64/$(BINARY)-v$(VERSION)-macos-x64)

linux-x64:
	@echo "🐧 Building Linux x64 [FULL VERSION] natively v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-x64
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(OUTPUT_DIR)/linux-x64/$(BINARY)-v$(VERSION)-linux-x64
	@$(call MD5,$(OUTPUT_DIR)/linux-x64/$(BINARY)-v$(VERSION)-linux-x64)

# 准备 Podman 编译镜像 (固化系统依赖)
prepare-podman-image:
	@if ! podman image exists $(BUILDER_IMAGE); then \
		echo "🛠️ Creating custom builder image $(BUILDER_IMAGE)..."; \
		printf "FROM docker.io/library/rust:latest\nRUN apt-get update && apt-get install -y g++-x86-64-linux-gnu && rustup target add x86_64-unknown-linux-gnu" | podman build -t $(BUILDER_IMAGE) -; \
	fi

# 提速版 Podman 构建 (系统依赖 + Rust 依赖双缓存)
linux-x64-with-podman: prepare-podman-image
	@echo "🐧 Building Linux x64 [FULL VERSION] via Podman v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-x64
	mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git
	podman run --rm \
		-v "$$(pwd)/../..:/workspace" \
		-v "$(CARGO_CACHE_DIR)/registry:/usr/local/cargo/registry" \
		-v "$(CARGO_CACHE_DIR)/git:/usr/local/cargo/git" \
		-w /workspace/cli/cowen \
		$(BUILDER_IMAGE) \
		bash -c "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc && \
			CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
			DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
			cargo build --release --target x86_64-unknown-linux-gnu && \
			cp target/x86_64-unknown-linux-gnu/release/$(BINARY) ../../bin/linux-x64/$(BINARY)-v$(VERSION)-linux-x64"
	@$(call MD5,$(OUTPUT_DIR)/linux-x64/$(BINARY)-v$(VERSION)-linux-x64)

linux-arm64:
	@echo "🐧 Building Linux ARM64 v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-arm64
	cargo zigbuild --release --target aarch64-unknown-linux-musl --no-default-features
	cp target/aarch64-unknown-linux-musl/release/$(BINARY) $(OUTPUT_DIR)/linux-arm64/$(BINARY)-v$(VERSION)-linux-arm64
	@$(call MD5,$(OUTPUT_DIR)/linux-arm64/$(BINARY)-v$(VERSION)-linux-arm64)

windows-x64:
	@echo "🪟 Building Windows x64 v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/windows-x64
	cargo build --release --target x86_64-pc-windows-msvc
	cp target/x86_64-pc-windows-msvc/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x64/$(BINARY)-v$(VERSION)-windows-x64.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x64/$(BINARY)-v$(VERSION)-windows-x64.exe)

# --- 打包目标 ---

package-macos-arm64: clean macos-arm64
	@echo "📦 Packaging macOS Apple Silicon..."
	mkdir -p pkg_root/usr/local/bin pkg_root/usr/local/share/doc/cowen $(OUTPUT_DIR)/macos-arm64/release
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY)-v$(VERSION)-macos-arm64 pkg_root/usr/local/bin/$(BINARY)
	cp CHANGELOG.md pkg_root/usr/local/share/doc/cowen/
	pkgbuild --identifier com.chanjet.$(BINARY).arm64 --version $(VERSION) --root pkg_root --install-location / $(OUTPUT_DIR)/macos-arm64/release/$(BINARY)-v$(VERSION)-macos-arm64.pkg
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/macos-arm64/release/$(BINARY)-v$(VERSION)-macos-arm64.pkg)

package-macos-x64: clean macos-x64
	@echo "📦 Packaging macOS Intel..."
	mkdir -p pkg_root/usr/local/bin pkg_root/usr/local/share/doc/cowen $(OUTPUT_DIR)/macos-x64/release
	cp $(OUTPUT_DIR)/macos-x64/$(BINARY)-v$(VERSION)-macos-x64 pkg_root/usr/local/bin/$(BINARY)
	cp CHANGELOG.md pkg_root/usr/local/share/doc/cowen/
	pkgbuild --identifier com.chanjet.$(BINARY).x64 --version $(VERSION) --root pkg_root --install-location / $(OUTPUT_DIR)/macos-x64/release/$(BINARY)-v$(VERSION)-macos-x64.pkg
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/macos-x64/release/$(BINARY)-v$(VERSION)-macos-x64.pkg)

package-linux-x64: clean linux-x64
	@echo "📦 Packaging Linux x64 (Native Build)..."
	@$(MAKE) impl-package-linux-x64

package-linux-x64-with-podman: clean linux-x64-with-podman
	@echo "📦 Packaging Linux x64 (Podman Build)..."
	@$(MAKE) impl-package-linux-x64

package-linux-arm64: clean linux-arm64
	@echo "📦 Packaging Linux ARM64..."
	mkdir -p $(OUTPUT_DIR)/linux-arm64/release
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-arm64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-arm64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-arm64/
	cd $(OUTPUT_DIR)/linux-arm64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-arm64.tar.gz $(BINARY)-v$(VERSION)-linux-arm64 install.sh README.txt CHANGELOG.md
	@$(call MD5,$(OUTPUT_DIR)/linux-arm64/release/$(BINARY)-v$(VERSION)-linux-arm64.tar.gz)

package-windows-x64: clean windows-x64
	@echo "📦 Packaging Windows x64..."
	mkdir -p $(OUTPUT_DIR)/windows-x64/release
	cp CHANGELOG.md ../cowen_setup/
	cd ../cowen_setup && cargo build --release --target x86_64-pc-windows-msvc
	cp ../cowen_setup/target/x86_64-pc-windows-msvc/release/cowen_setup.exe $(OUTPUT_DIR)/windows-x64/release/$(BINARY)-v$(VERSION)-windows-x64-setup.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x64/release/$(BINARY)-v$(VERSION)-windows-x64-setup.exe)

# 内部复用的 Linux 打包逻辑
impl-package-linux-x64:
	mkdir -p $(OUTPUT_DIR)/linux-x64/release
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-x64/
	cd $(OUTPUT_DIR)/linux-x64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-x64.tar.gz $(BINARY)-v$(VERSION)-linux-x64 install.sh README.txt CHANGELOG.md
	@$(call MD5,$(OUTPUT_DIR)/linux-x64/release/$(BINARY)-v$(VERSION)-linux-x64.tar.gz)

# --- 其他 ---

clean:
	cargo clean
	$(call RM,$(OUTPUT_DIR)/macos-arm64 $(OUTPUT_DIR)/macos-x64 $(OUTPUT_DIR)/linux-x64 $(OUTPUT_DIR)/linux-arm64 $(OUTPUT_DIR)/windows-x64 $(OUTPUT_DIR)/test)

install:
	@$(MAKE) macos-arm64
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY)-v$(VERSION)-macos-arm64 /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"

run:
	cargo run -- --help
