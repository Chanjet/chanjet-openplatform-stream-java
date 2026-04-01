BINARY=owenc
PKG=cli/owenc
OUTPUT_DIR=../../bin

# 生产环境默认值
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com

# 测试环境默认值
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build build-test build-mac-intel build-win64 build-linux-x64 build-linux-arm64 build-all test clean install package-mac

build:
	@echo "🍎 Building for current host (Apple Silicon) [FULL VERSION]..."
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)/macos-arm64
	cp target/release/owenc $(OUTPUT_DIR)/macos-arm64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-arm64/$(BINARY) 2>/dev/null || true
	@echo "✅ Build complete: $(OUTPUT_DIR)/macos-arm64/$(BINARY)"

build-test:
	APP_DIR_NAME=.owenc-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)/test
	cp target/release/owenc $(OUTPUT_DIR)/test/$(BINARY)-test
	@echo "🧪 Test build complete: $(OUTPUT_DIR)/test/$(BINARY)-test"

build-mac-intel:
	@echo "🍎 Building for macOS Intel (x86_64) [CORE VERSION]..."
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo build --release --target x86_64-apple-darwin --no-default-features
	mkdir -p $(OUTPUT_DIR)/macos-x64
	cp target/x86_64-apple-darwin/release/owenc $(OUTPUT_DIR)/macos-x64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-x64/$(BINARY) 2>/dev/null || true
	@echo "✅ Mac Intel Build complete: $(OUTPUT_DIR)/macos-x64/$(BINARY)"

build-win64:
	@echo "🪟 Building for Windows Intel (x86_64)..."
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo build --release --target x86_64-pc-windows-gnu --no-default-features
	mkdir -p $(OUTPUT_DIR)/windows-x64
	cp target/x86_64-pc-windows-gnu/release/owenc.exe $(OUTPUT_DIR)/windows-x64/$(BINARY).exe
	@echo "✅ Windows Build complete: $(OUTPUT_DIR)/windows-x64/$(BINARY).exe"

build-linux-x64:
	@echo "🐧 Building for Linux Intel (x86_64) [CORE VERSION]..."
	@# Use cargo zigbuild for MUSL static binary
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo zigbuild --release --target x86_64-unknown-linux-musl --no-default-features
	mkdir -p $(OUTPUT_DIR)/linux-x64
	cp target/x86_64-unknown-linux-musl/release/owenc $(OUTPUT_DIR)/linux-x64/$(BINARY)
	@echo "✅ Linux x64 Build complete: $(OUTPUT_DIR)/linux-x64/$(BINARY)"

build-linux-arm64:
	@echo "🐧 Building for Linux ARM64 [CORE VERSION]..."
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo zigbuild --release --target aarch64-unknown-linux-musl --no-default-features
	mkdir -p $(OUTPUT_DIR)/linux-arm64
	cp target/aarch64-unknown-linux-musl/release/owenc $(OUTPUT_DIR)/linux-arm64/$(BINARY)
	@echo "✅ Linux ARM64 Build complete: $(OUTPUT_DIR)/linux-arm64/$(BINARY)"

package-mac: build
	@echo "🍎 Packaging for macOS Apple Silicon..."
	mkdir -p pkg_root/usr/local/bin
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY) pkg_root/usr/local/bin/
	pkgbuild --identifier com.chanjet.owenc.arm64 \
	         --version 0.1.2 \
	         --root pkg_root \
	         --install-location / \
	         $(OUTPUT_DIR)/owenc-macos-arm64.pkg
	rm -rf pkg_root
	@echo "✅ macOS ARM64 Installer created: $(OUTPUT_DIR)/owenc-macos-arm64.pkg"

package-mac-x64: build-mac-intel
	@echo "🍎 Packaging for macOS Intel..."
	mkdir -p pkg_root/usr/local/bin
	cp $(OUTPUT_DIR)/macos-x64/$(BINARY) pkg_root/usr/local/bin/
	pkgbuild --identifier com.chanjet.owenc.x64 \
	         --version 0.1.2 \
	         --root pkg_root \
	         --install-location / \
	         $(OUTPUT_DIR)/owenc-macos-x64.pkg
	rm -rf pkg_root
	@echo "✅ macOS x64 Installer created: $(OUTPUT_DIR)/owenc-macos-x64.pkg"

package-win64: build-win64
	@echo "🪟 Packaging for Windows..."
	cd $(OUTPUT_DIR)/windows-x64 && \
	cp ../../bin/windows/install.ps1 . && \
	zip -r ../owenc-windows-x64.zip owenc.exe install.ps1
	@echo "✅ Windows Package created: $(OUTPUT_DIR)/owenc-windows-x64.zip"

package-linux-x64: build-linux-x64
	@echo "🐧 Packaging for Linux x64..."
	cd $(OUTPUT_DIR)/linux-x64 && \
	cp ../../bin/linux/install.sh . && \
	tar -czf ../owenc-linux-x64.tar.gz owenc install.sh
	@echo "✅ Linux x64 Package created: $(OUTPUT_DIR)/owenc-linux-x64.tar.gz"

package-linux-arm64: build-linux-arm64
	@echo "🐧 Packaging for Linux ARM64..."
	cd $(OUTPUT_DIR)/linux-arm64 && \
	cp ../../bin/linux/install.sh . && \
	tar -czf ../owenc-linux-arm64.tar.gz owenc install.sh
	@echo "✅ Linux ARM64 Package created: $(OUTPUT_DIR)/owenc-linux-arm64.tar.gz"

package-all: package-mac package-mac-x64 package-win64 package-linux-x64 package-linux-arm64
	@echo "📦 All packages created successfully in $(OUTPUT_DIR)/"

build-all: build build-mac-intel build-win64 build-linux-x64 build-linux-arm64
	@echo "📦 All targets built successfully in $(OUTPUT_DIR)/"

test:
	cargo test

clean:
	cargo clean
	rm -rf $(OUTPUT_DIR)/macos-arm64 $(OUTPUT_DIR)/macos-x64 $(OUTPUT_DIR)/windows-x64 $(OUTPUT_DIR)/linux-x64 $(OUTPUT_DIR)/linux-arm64

install: build
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"

run:
	cargo run -- --help
