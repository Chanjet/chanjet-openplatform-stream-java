BINARY=cowen
PKG=cli/cowen
OUTPUT_DIR=../../bin

# 生产环境默认值
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com

# 测试环境默认值
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build build-test build-mac-intel build-win64 build-linux-x64 build-linux-arm64 build-all test clean install package-all package-mac package-mac-x64 package-win64 package-linux-x64 package-linux-arm64

build:
	@echo "🍎 Building for current host (Apple Silicon) [FULL VERSION]..."
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)/macos-arm64
	cp target/release/$(BINARY) $(OUTPUT_DIR)/macos-arm64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-arm64/$(BINARY) 2>/dev/null || true
	@md5 -q $(OUTPUT_DIR)/macos-arm64/$(BINARY) > $(OUTPUT_DIR)/macos-arm64/$(BINARY).md5
	@echo "✅ Build complete: $(OUTPUT_DIR)/macos-arm64/$(BINARY) (MD5 generated)"

build-test:
	@echo "🧪 Building for test environment..."
	CARGO_BIN_NAME_OVERRIDE=$(BINARY)-test APP_DIR_NAME=.$(BINARY)-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)/test
	cp target/release/$(BINARY) $(OUTPUT_DIR)/test/$(BINARY)-test
	@codesign --force --deep --sign - $(OUTPUT_DIR)/test/$(BINARY)-test 2>/dev/null || true
	@md5 -q $(OUTPUT_DIR)/test/$(BINARY)-test > $(OUTPUT_DIR)/test/$(BINARY)-test.md5
	@echo "✅ Test build complete: $(OUTPUT_DIR)/test/$(BINARY)-test"

build-mac-intel:
	@echo "🍎 Building for macOS Intel (x86_64) [CORE VERSION]..."
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo build --release --target x86_64-apple-darwin --no-default-features
	mkdir -p $(OUTPUT_DIR)/macos-x64
	cp target/x86_64-apple-darwin/release/$(BINARY) $(OUTPUT_DIR)/macos-x64/$(BINARY)
	@codesign --force --deep --sign - $(OUTPUT_DIR)/macos-x64/$(BINARY) 2>/dev/null || true
	@md5 -q $(OUTPUT_DIR)/macos-x64/$(BINARY) > $(OUTPUT_DIR)/macos-x64/$(BINARY).md5
	@echo "✅ Mac Intel Build complete: $(OUTPUT_DIR)/macos-x64/$(BINARY)"

build-win64:
	@echo "🪟 Building for Windows Intel (x86_64)..."
	powershell -NoProfile -Command "$$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "if (-Not (Test-Path $(OUTPUT_DIR)/windows-x64)) { New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x64 | Out-Null }"
	powershell -NoProfile -Command "Copy-Item target/x86_64-pc-windows-msvc/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x64/$(BINARY).exe -Force"
	powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm MD5 $(OUTPUT_DIR)/windows-x64/$(BINARY).exe).Hash.ToLower(); Set-Content -Path $(OUTPUT_DIR)/windows-x64/$(BINARY).exe.md5 -Value $$hash"
	@echo "✅ Windows Build complete: $(OUTPUT_DIR)/windows-x64/$(BINARY).exe"

build-linux-x64:
	@echo "🐧 Building for Linux Intel (x86_64) [CORE VERSION]..."
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo zigbuild --release --target x86_64-unknown-linux-musl --no-default-features
	mkdir -p $(OUTPUT_DIR)/linux-x64
	cp target/x86_64-unknown-linux-musl/release/$(BINARY) $(OUTPUT_DIR)/linux-x64/$(BINARY)
	@md5 -q $(OUTPUT_DIR)/linux-x64/$(BINARY) > $(OUTPUT_DIR)/linux-x64/$(BINARY).md5
	@echo "✅ Linux x64 Build complete: $(OUTPUT_DIR)/linux-x64/$(BINARY)"

build-linux-arm64:
	@echo "🐧 Building for Linux ARM64 [CORE VERSION]..."
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo zigbuild --release --target aarch64-unknown-linux-musl --no-default-features
	mkdir -p $(OUTPUT_DIR)/linux-arm64
	cp target/aarch64-unknown-linux-musl/release/$(BINARY) $(OUTPUT_DIR)/linux-arm64/$(BINARY)
	@md5 -q $(OUTPUT_DIR)/linux-arm64/$(BINARY) > $(OUTPUT_DIR)/linux-arm64/$(BINARY).md5
	@echo "✅ Linux ARM64 Build complete: $(OUTPUT_DIR)/linux-arm64/$(BINARY)"

# Internal targets without clean
impl-package-mac: build
	@echo "🍎 Packaging for macOS Apple Silicon..."
	mkdir -p pkg_root/usr/local/bin $(OUTPUT_DIR)/macos-arm64/release
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY) pkg_root/usr/local/bin/
	pkgbuild --identifier com.chanjet.$(BINARY).arm64 --version 0.1.3 --root pkg_root --install-location / $(OUTPUT_DIR)/macos-arm64/release/$(BINARY)-macos-arm64.pkg
	rm -rf pkg_root
	@md5 -q $(OUTPUT_DIR)/macos-arm64/release/$(BINARY)-macos-arm64.pkg > $(OUTPUT_DIR)/macos-arm64/release/$(BINARY)-macos-arm64.pkg.md5

impl-package-mac-x64: build-mac-intel
	@echo "🍎 Packaging for macOS Intel..."
	mkdir -p pkg_root/usr/local/bin $(OUTPUT_DIR)/macos-x64/release
	cp $(OUTPUT_DIR)/macos-x64/$(BINARY) pkg_root/usr/local/bin/
	pkgbuild --identifier com.chanjet.$(BINARY).x64 --version 0.1.3 --root pkg_root --install-location / $(OUTPUT_DIR)/macos-x64/release/$(BINARY)-macos-x64.pkg
	rm -rf pkg_root
	@md5 -q $(OUTPUT_DIR)/macos-x64/release/$(BINARY)-macos-x64.pkg > $(OUTPUT_DIR)/macos-x64/release/$(BINARY)-macos-x64.pkg.md5

impl-package-win64: build-win64
	@echo "🪟 Packaging for Windows (Building Installer)..."
	powershell -NoProfile -Command "if (-Not (Test-Path $(OUTPUT_DIR)/windows-x64/release)) { New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x64/release | Out-Null }"
	powershell -NoProfile -Command "cd ../cowen_setup; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "Copy-Item ../cowen_setup/target/x86_64-pc-windows-msvc/release/cowen_setup.exe $(OUTPUT_DIR)/windows-x64/release/setup.exe -Force"
	powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm MD5 $(OUTPUT_DIR)/windows-x64/release/setup.exe).Hash.ToLower(); Set-Content -Path $(OUTPUT_DIR)/windows-x64/release/setup.exe.md5 -Value $$hash"

impl-package-linux-x64: build-linux-x64
	@echo "🐧 Packaging for Linux x64..."
	mkdir -p $(OUTPUT_DIR)/linux-x64/release
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x64/README.txt
	cd $(OUTPUT_DIR)/linux-x64 && tar -czf release/$(BINARY)-linux-x64.tar.gz $(BINARY) install.sh README.txt
	@md5 -q $(OUTPUT_DIR)/linux-x64/release/$(BINARY)-linux-x64.tar.gz > $(OUTPUT_DIR)/linux-x64/release/$(BINARY)-linux-x64.tar.gz.md5

impl-package-linux-arm64: build-linux-arm64
	@echo "🐧 Packaging for Linux ARM64..."
	mkdir -p $(OUTPUT_DIR)/linux-arm64/release
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-arm64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-arm64/README.txt
	cd $(OUTPUT_DIR)/linux-arm64 && tar -czf release/$(BINARY)-linux-arm64.tar.gz $(BINARY) install.sh README.txt
	@md5 -q $(OUTPUT_DIR)/linux-arm64/release/$(BINARY)-linux-arm64.tar.gz > $(OUTPUT_DIR)/linux-arm64/release/$(BINARY)-linux-arm64.tar.gz.md5

# Public targets with clean
package-mac: clean impl-package-mac
package-mac-x64: clean impl-package-mac-x64
package-win64: clean impl-package-win64
package-linux-x64: clean impl-package-linux-x64
package-linux-arm64: clean impl-package-linux-arm64

package-all: clean impl-package-mac impl-package-mac-x64 impl-package-win64 impl-package-linux-x64 impl-package-linux-arm64
	@echo "📦 All packages created successfully in $(OUTPUT_DIR)/"

build-all: build build-mac-intel build-win64 build-linux-x64 build-linux-arm64
	@echo "📦 All targets built successfully in $(OUTPUT_DIR)/"

test:
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) cargo test

clean:
	cargo clean
	rm -rf $(OUTPUT_DIR)/macos-arm64 $(OUTPUT_DIR)/macos-x64 $(OUTPUT_DIR)/windows-x64 $(OUTPUT_DIR)/linux-x64 $(OUTPUT_DIR)/linux-arm64 $(OUTPUT_DIR)/test

install: build
	cp $(OUTPUT_DIR)/macos-arm64/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"

run:
	cargo run -- --help
