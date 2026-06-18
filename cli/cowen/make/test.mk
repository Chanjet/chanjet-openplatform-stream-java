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
		bash -c "COWEN_SKIP_BROWSER=true RUSTFLAGS=\"-D warnings\" cargo test --release -j $(MAX_PARALLEL) --target x86_64-unknown-linux-gnu && COWEN_SKIP_BROWSER=true crates/app/cowen-cli/tests/runners/run_parallel.sh"

# 在 Windows PowerShell 环境下执行测试

# 在 Windows PowerShell 环境下执行测试
test-win: test-rust
	@echo "🧪 Running Full E2E Suite in Windows PowerShell..."
	powershell -NoProfile -ExecutionPolicy Bypass -Command "$$env:COWEN_SKIP_BROWSER='true'; crates/app/cowen-cli/tests/runners/run_suites.ps1"

# 在 macOS 原生环境执行并行测试
test-macos: local-db-up
	@echo "🧪 Running Full Coverage Testing Flow on macOS..."
	@bash scripts/run_tests_with_coverage.sh

