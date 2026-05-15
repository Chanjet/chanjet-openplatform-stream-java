# cli/cowen v0.3.1 任务拆解结构 (WBS)

基于 v0.3.1 的 PRD、HLD 和 LLD，我们将开发任务分解为以下有序的阶段 (Phases) 和具体的开发任务 (Tasks)。所有开发将严格遵循 TDD（测试驱动开发）流程。

## Phase 1: 基础设施准备 (Infrastructure Prep)
*   **Task 1.1**: 在 `cowen-common` 中引入独立的依赖以处理特定领域职责。
    *   `notify`: 负责跨平台的文件系统事件监听。
    *   `tokio::sync::watch`: 用于原子配置交换。
    *   `libloading`: 提供安全的动态库 FFI 加载能力。
    *   `prometheus`, `axum`: 提供监控与管理服务。
*   **Task 1.2**: 新建隔离的 Crate `cowen-search-embedding`。
    *   将现有 `cowen-ai` 中的所有 ONNX 和深度学习相关代码迁移至该 Crate。
    *   将其配置为可编译为 `cdylib` (动态链接库)。

## Phase 2: 配置热重载 (Config Hot-Reload)
*   **Task 2.1: `ConfigWatcher` 核心实现**
    *   实现基于 `notify` 的配置文件变更监听器。
    *   实现 `SIGHUP` 信号监听。
    *   利用 `tokio::sync::watch` 实现配置读取、合法性校验及广播更新的逻辑。
*   **Task 2.2: Daemon 集成**
    *   在 `DaemonService` 中挂载 `ConfigWatcher`。
    *   修改 `ProxyServer` 等后台任务，使其订阅配置更新，而不是持有静态的配置拷贝。
*   **Task 2.3: 日志级别动态调整**
    *   对接 `tracing-subscriber` 的 Reload 句柄，使得配置更新能实时改变日志级别。
*   **Task 2.4 (E2E)**: 实现 `tests/e2e/scripts/case_45_config_hot_reload.sh` 验证热重载不断流及日志级别的变化。

## Phase 3: 监控与健康 API (Metrics & Health API)
*   **Task 3.1: 监控服务搭建**
    *   在 `cowen-server` 中创建一个专用的 Axum Router，监听 `127.0.0.1` 特定端口。
    *   实现 `GET /health` 接口，组装基于 LLD 契约的 JSON 响应。
*   **Task 3.2: Prometheus 指标注册与导出**
    *   注册全局 Registry，定义所需的 Counter 和 Histogram。
    *   实现 `GET /metrics` 接口，导出为 Prometheus 文本格式。
*   **Task 3.3: 指标埋点**
    *   在 Proxy 处理层添加中间件，增加 `cowen_proxy_requests_total` 和耗时指标。
*   **Task 3.4 (E2E)**: 实现 `tests/e2e/scripts/case_46_metrics_health.sh` 验证端点连通性及指标累加逻辑。

## Phase 4: 环境自检工具 (System Doctor)
*   **Task 4.1: SPI 定义与基础框架**
    *   在 `cowen-common` 中定义 `Diagnostic` 接口及数据模型。
    *   实现一个调度引擎，支持并行运行多个探测器。
*   **Task 4.2: 具体诊断器实现**
    *   `StorageDiagnostic`: 检查数据库配置。
    *   `NetworkDiagnostic`: 检查网络连通性。
*   **Task 4.3: CLI 命令集成**
    *   新增 `cowen system doctor` 命令行入口。
    *   美化输出格式，展示状态 (OK/WARN/ERROR) 及修复建议。
*   **Task 4.4 (E2E)**: 实现 `tests/e2e/scripts/case_47_system_doctor.sh` 验证故意配错时能输出预期的 ERROR 和建议。

## Phase 5: API 搜索插件化 (Pluggable Search Engine)
*   **Task 5.1: 接口与内置实现**
    *   定义 `SearchProvider` Trait。
    *   实现内置的 `StringMatchProvider`。
*   **Task 5.2: 动态加载机制**
    *   实现通过 `libloading` 加载 `libcowen_search_embedding` 的逻辑。
    *   实现 FFI 安全的初始化与析构 (ABI 契约)。
    *   实现加载失败时的优雅降级 (Fallback) 逻辑。
*   **Task 5.3: CLI 改造与解耦**
    *   修改 `api list --search` 命令，使其通过统一的 `SearchManager` 路由调用。
    *   在构建系统中（如 `Cargo.toml` 和 `Makefile`）将 ONNX 依赖剥离出核心二进制，移入新 crate `cowen-search-embedding`（用于编译动态库）。
*   **Task 5.4 (E2E)**: 实现 `tests/e2e/scripts/case_48_search_plugin.sh` 验证降级警告及插件正常加载的行为。

## Phase 6: 文档与发布 (Release & Docs)
*   **Task 6.1**: 更新 `docs/COMMANDS.md` 补充新命令和配置项。
*   **Task 6.2**: 更新安装脚本/构建系统，以处理 `libcowen_search_embedding` 的可选分发。