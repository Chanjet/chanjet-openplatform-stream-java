# --- 数据库管理 (Database Management) ---

# 探测 Compose 工具
COMPOSE_BIN := $(shell command -v docker-compose 2> /dev/null || command -v docker compose 2> /dev/null)

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

# 专用于 Docker 测试的统一网络隔离模式
db-up-isolated: prepare-docker-machine
	@echo "🚀 Aggressively purging all Cowen test resources..."
	@docker stop cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	docker rm -f cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	docker network rm cowen-test-net 2>/dev/null || true; \
	sleep 2; \
	echo "🚀 Starting MySQL, PostgreSQL and Redis in a unified Docker Network..."; \
	docker network create cowen-test-net 2>/dev/null || true; \
	docker run -d --network cowen-test-net -p 3306:3306 --name cowen-mysql -e MYSQL_ROOT_PASSWORD=root -e MYSQL_DATABASE=cowen_test --health-cmd="mysqladmin ping -h localhost" --health-interval=2s mysql:8.0 || exit 1; \
	docker run -d --network cowen-test-net -p 5432:5432 --name cowen-postgres -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=password -e POSTGRES_DB=cowen_test --health-cmd="pg_isready -U postgres" --health-interval=2s postgres:15 || exit 1; \
	docker run -d --network cowen-test-net -p 6379:6379 --name cowen-redis --health-cmd="redis-cli ping" --health-interval=2s redis:7.0-alpine || exit 1; \
	echo "⌛ Waiting for databases in Docker Network to be ready..."; \
	MAX_WAIT=20; count=0; \
	while [ $$count -lt $$MAX_WAIT ]; do \
		HEALTHY_COUNT=$$(docker ps --filter "network=cowen-test-net" --filter "health=healthy" --format "{{.ID}}" | wc -l); \
		if [ $$HEALTHY_COUNT -ge 3 ]; then \
			echo -e "\n✅ Databases in Network are ready."; \
			exit 0; \
		fi; \
		printf "."; sleep 2; count=$$((count + 1)); \
	done; \
	echo -e "\n❌ TIMEOUT: Databases failed to become healthy."; \
	docker ps -a --filter "network=cowen-test-net"; \
	exit 1

db-down-isolated:
	@echo "🛑 Stopping and removing Docker test network..."
	@docker stop cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	docker rm -f cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	docker network rm cowen-test-net 2>/dev/null || true; \
	$(MAKE) stop-docker-machine

db-up: prepare-docker-machine
	@echo "🚀 Starting MySQL, PostgreSQL and Redis containers..."
	@if [ "$(COMPOSE_BIN)" = "" ]; then \
		echo "⚠️  No compose provider found, falling back to 'docker run'..."; \
		docker stop cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
		docker rm -f cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
		docker run -d --name cowen-mysql -p 3306:3306 -e MYSQL_ROOT_PASSWORD=root -e MYSQL_DATABASE=cowen_test --health-cmd="mysqladmin ping -h localhost" --health-interval=5s mysql:8.0; \
		docker run -d --name cowen-postgres -p 5432:5432 -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=password -e POSTGRES_DB=cowen_test --health-cmd="pg_isready -U postgres" --health-interval=5s postgres:15; \
		docker run -d --name cowen-redis -p 6379:6379 --health-cmd="redis-cli ping" --health-interval=5s redis:7.0-alpine; \
	else \
		echo "🚀 Using $(COMPOSE_BIN)..."; \
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v 2>/dev/null || true; \
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) up -d || (echo "❌ Failed to start containers." && exit 1); \
	fi; \
	echo "⌛ Waiting for databases to be ready..."; \
	MAX_WAIT=30; count=0; \
	until [ $$count -ge $$MAX_WAIT ] || [ $$(docker ps 2>/dev/null | grep 'healthy' | wc -l) -ge 3 ]; do \
		echo -n "."; sleep 2; count=$$((count + 1)); \
	done; \
	echo " ✅ Databases are ready."

db-down:
	@echo "🛑 Stopping and removing containers..."; \
	if [ "$(COMPOSE_BIN)" = "" ]; then \
		docker stop cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
		docker rm cowen-mysql cowen-postgres cowen-redis 2>/dev/null || true; \
	else \
		cd crates/app/cowen-cli/tests/infra && $(COMPOSE_BIN) down -v; \
	fi; \
	$(MAKE) stop-docker-machine


clean:
	cargo clean
	$(call RM,$(OUTPUT_DIR)/macos-aarch64 $(OUTPUT_DIR)/linux-x86_64 $(OUTPUT_DIR)/linux-aarch64 $(OUTPUT_DIR)/windows-x86 $(OUTPUT_DIR)/windows-x86_64 $(OUTPUT_DIR)/windows-x86_64-cross $(OUTPUT_DIR)/test target/cowen_tests_linux target/cowen_tests_macos target/cowen_tests)

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
check-cross: prepare-docker-image
	@echo "====== 开始多平台跨平台编译并行静态检查 ======"
	@rm -f .check_macos.log .check_linux.log .check_win.log
	@echo "⏳ 正在并行执行跨平台编译检查..."
	@trap '$(MAKE) stop-docker-machine' EXIT; \
	failed=0; \
	pid_macos=0; \
	pid_linux=0; \
	pid_win=0; \
	if [ "$(HOST_OS)" != "macos" ]; then \
		echo "⏳ [macOS] 启动静态编译检查..."; \
		cargo check --target aarch64-apple-darwin --workspace --all-targets -j 2 > .check_macos.log 2>&1 < /dev/null & pid_macos=$$!; \
	else \
		echo "⏭️ [macOS] 跳过当前主机平台的静态编译检查"; \
	fi; \
	if [ "$(HOST_OS)" != "linux" ]; then \
		echo "⏳ [Linux] 启动跨平台编译检查..."; \
		$(DOCKER_RUN_BASE) $(BUILDER_IMAGE) bash -c "cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets -j 2" > .check_linux.log 2>&1 < /dev/null & pid_linux=$$!; \
	else \
		echo "⏭️ [Linux] 跳过当前主机平台的跨平台编译检查"; \
	fi; \
	if [ "$(HOST_OS)" != "windows" ]; then \
		echo "⏳ [Windows] 启动跨平台编译检查..."; \
		DOCKER_DEFAULT_PLATFORM=linux/amd64 CARGO_TARGET_DIR=target/cross/win CROSS_NO_TTY=1 cross check --config 'build.rustc-wrapper=""' --target x86_64-pc-windows-gnu --workspace --all-targets -j 1 > .check_win.log 2>&1 < /dev/null & pid_win=$$!; \
	else \
		echo "⏭️ [Windows] 跳过当前主机平台的跨平台编译检查"; \
	fi; \
	if [ $$pid_macos -ne 0 ]; then \
		wait $$pid_macos || { echo "❌ [macOS] 检查失败:"; cat .check_macos.log; failed=1; }; \
	else \
		echo "✅ [macOS] 当前主机平台（跳过）"; \
	fi; \
	if [ $$pid_linux -ne 0 ]; then \
		wait $$pid_linux || { echo "❌ [Linux] 检查失败:"; cat .check_linux.log; failed=1; }; \
	else \
		echo "✅ [Linux] 当前主机平台（跳过）"; \
	fi; \
	if [ $$pid_win -ne 0 ]; then \
		wait $$pid_win || { echo "❌ [Windows] 检查失败:"; cat .check_win.log; failed=1; }; \
	else \
		echo "✅ [Windows] 当前主机平台（跳过）"; \
	fi; \
	if [ $$failed -eq 1 ]; then echo "====== 跨平台编译静态检查存在错误 ======"; exit 1; fi; \
	if [ $$pid_macos -ne 0 ]; then echo "✅ [macOS] 架构静态编译检查通过"; fi; \
	if [ $$pid_linux -ne 0 ]; then echo "✅ [Linux] 架构静态编译检查通过"; fi; \
	if [ $$pid_win -ne 0 ]; then echo "✅ [Windows] 架构静态编译检查通过"; fi; \
	echo "====== 所有平台跨平台编译静态检查完成 ======"

