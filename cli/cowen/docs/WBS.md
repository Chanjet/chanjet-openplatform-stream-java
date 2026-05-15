# cli/cowen v0.3.1 任务拆解结构 (WBS)

基于 v0.3.1 强调**物理 Crate 隔离**的 PRD、HLD 和 LLD，我们将开发任务分解为以下有序的阶段 (Phases) 和具体的开发任务 (Tasks)。所有开发将严格遵循 TDD（测试驱动开发）流程。

## Phase 1: 基础设施准备与 Crate 脚手架 (Infrastructure & Scaffolding)
*   **Task 1.1**: 更新工作区 `Cargo.toml`，注册新的 Crate 成员。
*   **Task 1.2**: 使用 `cargo new --lib` 创建以下隔离的内部 Crate：
    *   `crates/cowen-config`
    *   `crates/cowen-monitor`
    *   `crates/cowen-doctor`
    *   `crates/cowen-search`
    *   `crates/cowen-search-embedding` (剥离现有的 `cowen-ai`)

## Phase 2: 配置热重载 (cowen-config)
*   **Task 2.1: `cowen-config` 核心实现**
    *   引入 `notify` 和 `tokio` 依赖。
    *   实现基于 `notify` 的配置文件变更监听器和 `SIGHUP` 信号监听。
    *   利用 `tokio::sync::watch` 对外暴露配置订阅能力。
*   **Task 2.2: Daemon 集成与日志级别动态调整**
    *   在 `cowen-server` (Daemon) 中引入 `cowen-config`。
    *   修改 `ProxyServer` 等长连接后台任务，使其订阅配置更新。
    *   对接 `tracing-subscriber` 的 Reload 句柄，随配置更新动态改变日志级别。
*   **Task 2.3 (E2E)**: 实现 `tests/e2e/scripts/case_45_config_hot_reload.sh` 验证热重载不断流及日志级别的变化。

## Phase 3: 监控与健康 API (cowen-monitor)
*   **Task 3.1: `cowen-monitor` 服务搭建**
    *   引入 `prometheus` 和 `axum`。
    *   实现独立运行在本地端口的 HTTP 服务，暴露 `/health` 和 `/metrics`。
*   **Task 3.2: 宏定义与无侵入埋点**
    *   在 `cowen-monitor` 导出打点宏 (如 `counter!()`)。
    *   在 `cowen-server` (Proxy 处理层等) 引入宏进行无侵入式指标统计。
*   **Task 3.3 (E2E)**: 实现 `tests/e2e/scripts/case_46_metrics_health.sh` 验证端点连通性及指标累加逻辑。

## Phase 4: 环境自检工具 (cowen-doctor)
*   **Task 4.1: `cowen-doctor` SPI 与调度台**
    *   定义 `Diagnostic` Trait 及 `DiagnosticResult` 模型。
    *   实现并发诊断调度引擎。
*   **Task 4.2: 诊断器实现与装配**
    *   在 `cowen` 主项目中，依赖底层网络/存储模块实现具体的网络探测 (`NetworkDiagnostic`) 和数据库探测 (`StorageDiagnostic`)。
    *   将这些探测器注册到 `cowen-doctor`。
*   **Task 4.3: CLI 命令集成**
    *   新增 `cowen system doctor` 命令行入口，格式化输出。
*   **Task 4.4 (E2E)**: 实现 `tests/e2e/scripts/case_47_system_doctor.sh` 验证故意配错时能输出预期的 ERROR 和建议。

## Phase 5: API 搜索插件化 (cowen-search)
*   **Task 5.1: `cowen-search` SPI 与内置实现**
    *   定义 `SearchProvider` Trait。
    *   实现内置基于字符串匹配的 `StringMatchProvider`。
*   **Task 5.2: `cowen-search-embedding` 动态库化**
    *   迁移 ONNX 模型逻辑，配置为 `cdylib` 编译目标。
    *   建立 C ABI 边界 (`v1_init`, `v1_free`)。
*   **Task 5.3: 动态加载机制与 Fallback**
    *   在 `cowen-search` 中引入 `libloading`，实现运行时加载动态库的安全逻辑。
    *   实现加载失败时的优雅降级 (Fallback) 逻辑。
    *   修改 `api list --search` 命令路由至 `cowen-search`。
*   **Task 5.4 (E2E)**: 实现 `tests/e2e/scripts/case_48_search_plugin.sh` 验证降级警告及插件正常加载的行为。

## Phase 6: 文档与发布 (Release & Docs)
*   **Task 6.1**: 更新 `docs/COMMANDS.md` 补充新命令和配置项。
*   **Task 6.2**: 更新安装脚本/Makefile，处理 `libcowen_search_embedding` 的可选分发。