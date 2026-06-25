# --- 核心构建目标 ---

build-plugins:
	@echo "🛠️ Building search-embedding plugin (Release)..."
	cargo build --release -p cowen-search-embedding
	@echo "🛠️ Building mcp-plugin (Release)..."
	cargo build --release -p cowen-mcp-plugin
	@echo "🔐 Signing plugins natively..."
ifeq ($(OS),Windows_NT)
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/release/libcowen_search_embedding.exe') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json } }"
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/release/cowen-mcp-plugin.exe') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/cowen-mcp-plugin.exe --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/cowen-mcp-plugin.bundle --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json } }"
else
	@if [ -f "dist_assets/keys/official_dev.pk8" ]; then \
		if [ -f "target/release/libcowen_search_embedding" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/libcowen_search_embedding --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json; fi; \
		if [ -f "target/release/cowen-mcp-plugin" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/cowen-mcp-plugin --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/cowen-mcp-plugin.bundle --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json; fi; \
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
	@$(call SHA256,$(OUTPUT_DIR)/macos-aarch64/$(BINARY))
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
	@$(call SHA256,$(OUTPUT_DIR)/test/$(BINARY))
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
	@$(call SHA256,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))

# 确保 Docker 虚拟机 (Colima) 已启动并响应 (仅 macOS)
prepare-docker-machine:
ifeq ($(HOST_OS),macos)
	@if ! colima status >/dev/null 2>&1; then \
		echo "🚀 Starting Colima machine..."; \
		colima start --cpu 4 --memory 8 || true; \
		echo "⌛ Waiting for Docker socket to be ready..."; \
		sleep 5; \
	fi
	@if ! docker info > /dev/null 2>&1; then \
		echo "⚠️  Docker is not responsive. Please check Colima or Docker Desktop status."; \
	fi
endif

# 停止 Docker 虚拟机 (仅 macOS Colima)
stop-docker-machine:
ifeq ($(HOST_OS),macos)
	@echo "🟢 Keeping Colima machine running for subsequent tasks..."
endif

# 哨兵文件：记录镜像最后一次成功构建的时间
.builder_image_built: Dockerfile.builder
	@echo "🛠️ Building/Updating custom builder image $(BUILDER_IMAGE) (Native/Cross)..."
	docker build -t $(BUILDER_IMAGE) - < Dockerfile.builder
	@touch .builder_image_built

# 准备 Docker 编译镜像
# 如果 Dockerfile.builder 发生变更，make 会自动重新执行 .builder_image_built 目标
prepare-docker-image: prepare-docker-machine .builder_image_built
	@if ! docker image inspect $(BUILDER_IMAGE) >/dev/null 2>&1; then \
		echo "🐳 Image $(BUILDER_IMAGE) missing from Docker, rebuilding..."; \
		$(MAKE) -W Dockerfile.builder .builder_image_built; \
	fi

# --- Docker 共享执行宏 (Shared Docker Invocation) ---
# 定义统一的挂载与环境变量，确保打包与测试复用同一套缓存与构建产物
# 增加 --memory 与 --shm-size 确保容器有足够的运行资源，防止 OOM
DOCKER_RUN_BASE = docker run --rm \
	--memory=12g \
	--shm-size=2g \
	-v "$$(pwd)/../..:/workspace" \
	-v "$(CARGO_CACHE_DIR)/registry:/root/.cargo/registry" \
	-v "$(CARGO_CACHE_DIR)/git:/root/.cargo/git" \
	-v "$(CARGO_CACHE_DIR)/target:/workspace/cli/cowen/target_docker" \
	-w /workspace/cli/cowen \
	-e CARGO_TARGET_DIR=target_docker \
	-e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
	-e BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	-e DEF_OPENAPI_URL=$(PROD_OPENAPI) \
	-e DEF_STREAM_URL=$(PROD_STREAM) \
	-e CARGO_BIN_NAME_OVERRIDE=$(BINARY) \
	-e APP_DIR_NAME=.$(BINARY) \
	-e CARGO_PROFILE_RELEASE_LTO=false \
	-e CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256

# 提速版 Docker 构建 (系统依赖 + Rust 依赖双缓存)
linux-x86_64-with-docker: prepare-docker-image
	@echo "🐧 Building Linux x86_64 [FULL VERSION] via Docker v$(VERSION)..."
	@mkdir -p $(OUTPUT_DIR)/linux-x86_64
	@mkdir -p $(CARGO_CACHE_DIR)/registry $(CARGO_CACHE_DIR)/git $(CARGO_CACHE_DIR)/target
	@trap '$(MAKE) stop-docker-machine' EXIT; \
	$(DOCKER_RUN_BASE) \
		$(BUILDER_IMAGE) \
		bash -c "cargo build --release -j $(MAX_PARALLEL) -p cowen-signer && \
			cargo build --release -j $(MAX_PARALLEL) --target x86_64-unknown-linux-gnu -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-mcp-plugin && \
			target_docker/release/cowen-signer sign-plugin --dylib target_docker/x86_64-unknown-linux-gnu/release/libcowen_search_embedding --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target_docker/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json && \
			target_docker/release/cowen-signer sign-plugin --dylib target_docker/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target_docker/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin.bundle --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json && \
			cp target_docker/x86_64-unknown-linux-gnu/release/$(BINARY) ../../bin/linux-x86_64/$(BINARY) && \
			cp target_docker/x86_64-unknown-linux-gnu/release/cowen-daemon ../../bin/linux-x86_64/cowen-daemon && \
			cp target_docker/x86_64-unknown-linux-gnu/release/libcowen_search_embedding ../../bin/linux-x86_64/ && \
			cp target_docker/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle ../../bin/linux-x86_64/ && \
			cp target_docker/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin ../../bin/linux-x86_64/ && \
			cp target_docker/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin.bundle ../../bin/linux-x86_64/" && \
	$(call MD5,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call SHA256,$(OUTPUT_DIR)/linux-x86_64/$(BINARY)) && \
	$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))

linux-x86_64-cross: build-system-plugins
	@echo "🐧 Cross-compiling Linux x86_64 [FULL VERSION] natively via cargo-zigbuild v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/linux-x86_64
	cargo build --release -p cowen-signer
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
	DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	cargo zigbuild --release --target x86_64-unknown-linux-gnu -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-mcp-plugin
	target/release/cowen-signer sign-plugin --dylib target/x86_64-unknown-linux-gnu/release/libcowen_search_embedding --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json
	target/release/cowen-signer sign-plugin --dylib target/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin.bundle --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(OUTPUT_DIR)/linux-x86_64/$(BINARY)
	cp target/x86_64-unknown-linux-gnu/release/cowen-daemon $(OUTPUT_DIR)/linux-x86_64/cowen-daemon
	cp target/x86_64-unknown-linux-gnu/release/libcowen_search_embedding $(OUTPUT_DIR)/linux-x86_64/
	cp target/x86_64-unknown-linux-gnu/release/libcowen_search_embedding.bundle $(OUTPUT_DIR)/linux-x86_64/
	cp target/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin $(OUTPUT_DIR)/linux-x86_64/
	cp target/x86_64-unknown-linux-gnu/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/linux-x86_64/
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))
	@$(call SHA256,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-x86_64/$(BINARY))


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
	@$(call SHA256,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))
	@$(call VERIFY_BIN,$(OUTPUT_DIR)/linux-aarch64/$(BINARY))

windows-x86_64: build-plugins build-system-plugins
	@echo "🪟 Building Windows x86_64 v$(VERSION)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "$$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; $$env:BUILTIN_CLIENT_ID='$(OFFICIAL_APP_KEY)'; cargo build --release --target x86_64-pc-windows-msvc -p cowen-cli -p cowen-daemon"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/$(BINARY).exe -Destination $(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/cowen-daemon.exe -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-daemon.exe -Force"
	powershell -NoProfile -Command "if (Test-Path target/release/libcowen_search_embedding.exe) { cargo run --release -p cowen-signer -- sign-plugin --dylib target/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json; Copy-Item -Path target/release/libcowen_search_embedding.exe -Destination $(OUTPUT_DIR)/windows-x86_64/libcowen_search_embedding.exe -Force; Copy-Item -Path target/release/libcowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/libcowen_search_embedding.bundle -Force }"
	powershell -NoProfile -Command "if (Test-Path target/release/cowen-mcp-plugin.exe) { Copy-Item -Path target/release/cowen-mcp-plugin.exe -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-mcp-plugin.exe -Force; Copy-Item -Path target/release/cowen-mcp-plugin.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/cowen-mcp-plugin.bundle -Force }"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)
	@$(call SHA256,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)

windows-x86-cross: build-system-plugins
	@echo "🪟 Cross-compiling Windows x86 (32-bit) v$(VERSION) via cargo-zigbuild..."
	mkdir -p $(OUTPUT_DIR)/windows-x86
	WINAPI_X86_LIB_DIR=$$(cargo metadata --format-version 1 | tr ',' '\n' | grep winapi-i686-pc-windows-gnu | grep src_path | head -n 1 | cut -d '"' -f 4 | sed 's/\/src\/lib.rs/\/lib/g'); \
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
	DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	RUSTFLAGS="-L $$WINAPI_X86_LIB_DIR" \
	cargo zigbuild --release --target i686-pc-windows-gnu --no-default-features -p cowen-cli -p cowen-daemon
	cp target/i686-pc-windows-gnu/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x86/$(BINARY).exe
	cp target/i686-pc-windows-gnu/release/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86/cowen-daemon.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x86/$(BINARY).exe)
	@$(call SHA256,$(OUTPUT_DIR)/windows-x86/$(BINARY).exe)

windows-x86_64-cross: build-system-plugins
	@echo "🪟 Cross-compiling Windows x86_64 [FULL VERSION] natively via cargo-zigbuild v$(VERSION)..."
	mkdir -p $(OUTPUT_DIR)/windows-x86_64
	cargo build --release -p cowen-signer
	WINAPI_X64_LIB_DIR=$$(cargo metadata --format-version 1 | tr ',' '\n' | grep winapi-x86_64-pc-windows-gnu | grep src_path | head -n 1 | cut -d '"' -f 4 | sed 's/\/src\/lib.rs/\/lib/g'); \
	CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) \
	DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) \
	BUILTIN_CLIENT_ID=$(OFFICIAL_APP_KEY) \
	RUSTFLAGS="-L $$WINAPI_X64_LIB_DIR" \
	cargo zigbuild --release --target x86_64-pc-windows-gnu -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-mcp-plugin
	target/release/cowen-signer sign-plugin --dylib target/x86_64-pc-windows-gnu/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-pc-windows-gnu/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json
	target/release/cowen-signer sign-plugin --dylib target/x86_64-pc-windows-gnu/release/cowen-mcp-plugin.exe --name cowen-mcp-plugin --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-pc-windows-gnu/release/cowen-mcp-plugin.bundle --manifest-file crates/plugins/cowen-mcp-plugin/plugin.json
	cp target/x86_64-pc-windows-gnu/release/$(BINARY).exe $(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe
	cp target/x86_64-pc-windows-gnu/release/cowen-daemon.exe $(OUTPUT_DIR)/windows-x86_64/cowen-daemon.exe
	cp target/x86_64-pc-windows-gnu/release/libcowen_search_embedding.exe $(OUTPUT_DIR)/windows-x86_64/
	cp target/x86_64-pc-windows-gnu/release/libcowen_search_embedding.bundle $(OUTPUT_DIR)/windows-x86_64/
	cp target/x86_64-pc-windows-gnu/release/cowen-mcp-plugin.exe $(OUTPUT_DIR)/windows-x86_64/
	cp target/x86_64-pc-windows-gnu/release/cowen-mcp-plugin.bundle $(OUTPUT_DIR)/windows-x86_64/
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)
	@$(call SHA256,$(OUTPUT_DIR)/windows-x86_64/$(BINARY).exe)

windows-plugin-x64:
	@echo "🪟 Building Windows x86_64 AI plugin (MSVC Release)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64"
	powershell -NoProfile -Command "cargo build --release --target x86_64-pc-windows-msvc -p cowen-search-embedding"
	powershell -NoProfile -Command "cargo run --release -p cowen-signer -- sign-plugin --dylib target/x86_64-pc-windows-msvc/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/x86_64-pc-windows-msvc/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/libcowen_search_embedding.exe -Destination $(OUTPUT_DIR)/windows-x86_64/libcowen_search_embedding.exe -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/x86_64-pc-windows-msvc/release/libcowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86_64/libcowen_search_embedding.bundle -Force"

windows-plugin-x86:
	@echo "🪟 Building Windows x86 (32-bit) AI plugin (MSVC Release)..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86"
	powershell -NoProfile -Command "cargo build --release --target i686-pc-windows-msvc -p cowen-search-embedding"
	powershell -NoProfile -Command "cargo run --release -p cowen-signer -- sign-plugin --dylib target/i686-pc-windows-msvc/release/libcowen_search_embedding.exe --name cowen-search-embedding --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/i686-pc-windows-msvc/release/libcowen_search_embedding.bundle --manifest-file crates/plugins/cowen-search-embedding/plugin.json"
	powershell -NoProfile -Command "Copy-Item -Path target/i686-pc-windows-msvc/release/libcowen_search_embedding.exe -Destination $(OUTPUT_DIR)/windows-x86/libcowen_search_embedding.exe -Force"
	powershell -NoProfile -Command "Copy-Item -Path target/i686-pc-windows-msvc/release/libcowen_search_embedding.bundle -Destination $(OUTPUT_DIR)/windows-x86/libcowen_search_embedding.bundle -Force"

# --- System Plugins ---

build-system-plugins:
	@echo "🛠️ Building builtin Wasm system plugins (Release)..."
	rustup target add wasm32-wasip1 || true
	cargo build --release -p cowen-wasm-auth-selfbuilt -p cowen-wasm-auth-storeapp --target wasm32-wasip1
	@echo "🔐 Signing Wasm system plugins..."
ifeq ($(OS),Windows_NT)
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm --name cowen-wasm-auth-selfbuilt --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle --manifest-file crates/plugins/cowen-wasm-auth-selfbuilt/plugin.json } }"
	@powershell -NoProfile -Command "if (Test-Path 'dist_assets/keys/official_dev.pk8') { if (Test-Path 'target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm') { cargo run --release -p cowen-signer -- sign-plugin --dylib target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm --name cowen-wasm-auth-storeapp --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle --manifest-file crates/plugins/cowen-wasm-auth-storeapp/plugin.json } }"
else
	@if [ -f "dist_assets/keys/official_dev.pk8" ]; then \
		if [ -f "target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm --name cowen-wasm-auth-selfbuilt --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle --manifest-file crates/plugins/cowen-wasm-auth-selfbuilt/plugin.json; fi; \
		if [ -f "target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm" ]; then cargo run --release -p cowen-signer -- sign-plugin --dylib target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm --name cowen-wasm-auth-storeapp --version $(VERSION) --dev-key dist_assets/keys/official_dev.pk8 --dev-cert dist_assets/keys/official_dev_cert.json --out-bundle target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle --manifest-file crates/plugins/cowen-wasm-auth-storeapp/plugin.json; fi; \
	fi
endif

dev-setup: build-system-plugins
	@echo "🛠️ Copying system plugins for local development..."
	mkdir -p ~/.cowen/system_plugins
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm ~/.cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle ~/.cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm ~/.cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle ~/.cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_custom.wasm ~/.cowen/system_plugins/ || true
	@echo "✅ System plugins ready in ~/.cowen/system_plugins/"
