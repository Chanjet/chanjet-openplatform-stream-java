BINARY=owenc
PKG=cli/owenc
OUTPUT_DIR=../../bin

# 生产环境默认值
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com

# 测试环境默认值
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build build-test test clean install

build:
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)
	cp target/release/owenc $(OUTPUT_DIR)/$(BINARY)
	@# macOS 26+ requires code signing to avoid UE+ hang
	@codesign --force --deep --sign - $(OUTPUT_DIR)/$(BINARY) 2>/dev/null || true
	@echo "✅ Build complete: $(OUTPUT_DIR)/$(BINARY)"

build-test:
	APP_DIR_NAME=.owenc-test DEF_OPENAPI_URL=$(TEST_OPENAPI) DEF_STREAM_URL=$(TEST_STREAM) cargo build --release
	mkdir -p $(OUTPUT_DIR)
	cp target/release/owenc $(OUTPUT_DIR)/$(BINARY)-test
	@codesign --force --deep --sign - $(OUTPUT_DIR)/$(BINARY)-test 2>/dev/null || true
	@echo "🧪 Test build complete: $(OUTPUT_DIR)/$(BINARY)-test"

build-win64:
	@echo "🪟 Building for Windows 11 Intel (x86_64)..."
	APP_DIR_NAME=.owenc DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	cargo build --release --target x86_64-pc-windows-gnu --no-default-features
	mkdir -p $(OUTPUT_DIR)/windows
	cp target/x86_64-pc-windows-gnu/release/owenc.exe $(OUTPUT_DIR)/windows/$(BINARY).exe
	@echo "✅ Windows Build complete: $(OUTPUT_DIR)/windows/$(BINARY).exe"

test:
	cargo test

clean:
	cargo clean
	rm -f $(OUTPUT_DIR)/$(BINARY)

install: build
	cp $(OUTPUT_DIR)/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "🚀 Installed to /usr/local/bin/$(BINARY)"
	@echo "⚙️ Generating auto-completion scripts..."
	@mkdir -p /usr/local/etc/bash_completion.d
	@$(OUTPUT_DIR)/$(BINARY) completion bash > /usr/local/etc/bash_completion.d/owenc
	@mkdir -p /usr/local/share/zsh/site-functions
	@$(OUTPUT_DIR)/$(BINARY) completion zsh > /usr/local/share/zsh/site-functions/_owenc
	@mkdir -p /usr/local/share/fish/vendor_completions.d
	@$(OUTPUT_DIR)/$(BINARY) completion fish > /usr/local/share/fish/vendor_completions.d/owenc.fish
	@echo "✅ Auto-completion scripts installed for Bash, Zsh, and Fish."

run:
	cargo run -- --help
