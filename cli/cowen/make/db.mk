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
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v 2>/dev/null || true; \
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) up -d || (echo "❌ Failed to start containers." && exit 1); \
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
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v; \
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
	@echo "🚀 Installing plugins to ~/.cowen/plugins/..."
	@if [ -n "$$SUDO_USER" ]; then \
		REAL_HOME=$$(eval echo "~$$SUDO_USER"); \
		mkdir -p $$REAL_HOME/.cowen/plugins; \
		rm -f $$REAL_HOME/.cowen/plugins/cowen-mcp-plugin $$REAL_HOME/.cowen/plugins/cowen-mcp-plugin.bundle; \
		rm -f $$REAL_HOME/.cowen/plugins/libcowen_search_embedding $$REAL_HOME/.cowen/plugins/libcowen_search_embedding.bundle; \
		cp target/release/cowen-mcp-plugin $$REAL_HOME/.cowen/plugins/ || true; \
		cp target/release/cowen-mcp-plugin.bundle $$REAL_HOME/.cowen/plugins/ || true; \
		cp target/release/libcowen_search_embedding $$REAL_HOME/.cowen/plugins/ || true; \
		cp target/release/libcowen_search_embedding.bundle $$REAL_HOME/.cowen/plugins/ || true; \
		chown -R $$SUDO_USER $$REAL_HOME/.cowen; \
	else \
		mkdir -p $(HOME)/.cowen/plugins; \
		rm -f $(HOME)/.cowen/plugins/cowen-mcp-plugin $(HOME)/.cowen/plugins/cowen-mcp-plugin.bundle; \
		rm -f $(HOME)/.cowen/plugins/libcowen_search_embedding $(HOME)/.cowen/plugins/libcowen_search_embedding.bundle; \
		cp target/release/cowen-mcp-plugin $(HOME)/.cowen/plugins/ || true; \
		cp target/release/cowen-mcp-plugin.bundle $(HOME)/.cowen/plugins/ || true; \
		cp target/release/libcowen_search_embedding $(HOME)/.cowen/plugins/ || true; \
		cp target/release/libcowen_search_embedding.bundle $(HOME)/.cowen/plugins/ || true; \
	fi
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

