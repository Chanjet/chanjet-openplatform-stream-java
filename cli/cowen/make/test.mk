# --- 测试目标 (Test Targets) ---

quality-gate:
	@echo "🎨 Auto-formatting code..."
	@cargo fmt --all
	@bash scripts/quality_gate.sh

test-rust: quality-gate
	@echo "🧪 Running Rust internal and integration tests..."
	RUSTFLAGS="-D warnings" cargo build -p cowen-cli -p cowen-daemon
	COWEN_SKIP_BROWSER=true RUSTFLAGS="-D warnings" cargo test

# 自动根据环境运行测试
test: quality-gate
ifeq ($(HOST_OS),macos)
	@$(MAKE) test-macos
else ifeq ($(HOST_OS),linux)
	@$(MAKE) test-linux
else ifeq ($(HOST_OS),windows)
	@$(MAKE) test-win
endif
	@$(MAKE) check-cross

# 在 Linux 容器中执行并行 E2E (Docker) - 封闭纯净环境
# 在 Linux 容器中执行并行 E2E (Docker) - All-in-One 封闭容器模式
# 数据库服务内置于测试容器中，无需外部 db-up，无需 --network host
test-linux: quality-gate prepare-docker-machine prepare-docker-image
	@echo "🧪 Running Full Test Suite in Linux container (All-in-One mode)..."
	@trap '$(MAKE) stop-docker-machine' EXIT; \
	$(DOCKER_RUN_BASE) \
		-it \
		-e DB_HOST=127.0.0.1 \
		-e MAX_PARALLEL=$(MAX_PARALLEL) \
		-e QEMU_LD_PREFIX=/usr/x86_64-linux-gnu/ \
		$(BUILDER_IMAGE) \
		bash -c "CARGO_BUILD_TARGET=x86_64-unknown-linux-gnu bash scripts/run_tests_with_coverage.sh"



# 在 Windows PowerShell 环境下执行测试

# 在 Windows PowerShell 环境下执行测试
test-win: local-db-up
	@echo "🧪 Running Full Coverage Testing Flow on Windows..."
	@bash scripts/run_tests_with_coverage.sh

# 在 macOS 原生环境执行并行测试
test-macos: local-db-up
	@echo "🧪 Running Full Coverage Testing Flow on macOS..."
	@bash scripts/run_tests_with_coverage.sh


# 在 macOS 下使用原生 Wine 运行 Windows 版本的 E2E 测试
test-windows-on-mac:
	@if command -v wine64 >/dev/null 2>&1; then \
		$(MAKE) _test-windows-on-mac-native WINE_CMD=wine64; \
	elif command -v wine >/dev/null 2>&1; then \
		echo "⚠️ 'wine64' not found, but 'wine' is available. Attempting native Wine tests..."; \
		$(MAKE) _test-windows-on-mac-native WINE_CMD=wine; \
	else \
		echo "❌ 'wine64' or 'wine' not found. Please install wine to run Windows E2E tests on macOS."; \
		exit 1; \
	fi

# 默认串行执行以防止 Wine 进程冲突和 CPU 负载过高，支持从命令行覆盖
MAX_PARALLEL ?= 1

_test-windows-on-mac-native: local-db-up
	@echo "🧪 Running Windows Test Suite natively via $(WINE_CMD) on macOS..."
	@CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=$(WINE_CMD) \
	CARGO_BUILD_TARGET=x86_64-pc-windows-gnu \
	OS_NAME=windows-cross \
	MAX_PARALLEL=$(MAX_PARALLEL) \
	bash scripts/run_tests_with_coverage.sh

