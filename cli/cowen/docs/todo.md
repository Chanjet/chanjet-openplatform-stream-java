# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🔴 P0: 紧急且关键 (Critical & Urgent)
*暂无未完成事项*

## 🟠 P1: 高优先级 (High Priority)
*暂无未完成事项*

## 🔵 P2: 中低优先级 (Medium/Low Priority)
- [x] **插件管理命令 (Plugin Management)**: 在 CLI 中增加 `cowen plugins` 命令组，支持对 `~/.cowen/plugins/` 目录下的插件进行快速管理。
    - [x] `cowen plugins list`: 扫描目录并列出可用的插件选手（通过启发式检测）。
    - [x] `cowen plugins enable/disable <NAME>`: 一键开关插件，自动更新 `app.yaml` 映射，实现真正的即插即用。
- [ ] ~~**拆解 Makefile**~~: 简化 `Makefile` 逻辑，将平台适配和容器管理逻辑拆分为独立的脚本 (Cancelled for v0.3.4 freeze).

---

## 🗄️ 已归档完成事项 (Archived Completed Items)

### 🔴 P0: 紧急且关键 (Critical & Urgent)
- [x] 🎯 **消除潜在 Panic** (已规划，并入 v0.3.1 PRD 2.5): 修复 `forwarder.rs` 中 `DlqStore::new` 的 `unwrap` 调用。
    *   **实现建议**: 将 `Forwarder::new` 修改为返回 `CowenResult<Self>`，并使用 `?` 向上层传播存储初始化错误。在 `bridge.rs` 调用处增加错误捕获与日志记录，确保存储故障时能以非崩溃方式提示用户或执行降级逻辑。
- [x] 🎯 **动态 Token 检查策略** (已规划，并入 v0.3.1 PRD 2.6): 替换 `renewer.rs` 和 `bridge.rs` 中硬编码的 10 分钟轮询间隔。
    *   **实现建议**: 
        1. **动态间隔计算**: 计算公式 `next_check = (expires_at - now) * 0.8`（或提前 15 分钟），取其与最小检查间隔（如 30s）的较大值。
        2. **引入抖动 (Jitter)**: 在计算结果上增加 `±(rand(0..60))` 秒的随机偏移，防止大量客户端在同一时刻触发刷新请求。
        3. **上限保护**: 设置最大检查间隔（如 1 小时），确保状态最终一致性。

### 🟠 P1: 高优先级 (High Priority)
- [x] **迁移核心配置至应用全局 (app.yaml)**: 
    *   **现状**: `security`, `log`, `openapi_url`, `stream_url`, `search` 等配置目前在 `Config` (Profile 级别) 中定义。
    *   **修改建议**: 应该作为应用级配置移入 `AppConfig`，确保这些基础架构配置在所有 Profile 间共享，避免重复配置。
- [x] **重构授权同步机制**: 替换 `orchestrator.rs` 中基于日志轮询的同步方式。现已实现基于 Monitor API (IPC) 的实时进度同步与进度条展示。
- [x] **实现优雅关机 (Graceful Shutdown)**: 显式跟踪所有异步任务（如 Token 交换、事件处理），确保守护进程退出时能安全回收资源。
- [x] **优化 DLQ 重试逻辑**: 改进 `Forwarder::retry_message`，实现分页查询与按 ID 精确检索，解决了 OOM 风险。
- [x] **重构 Worker 生命周期管理**: 将 `WorkerManager` 中的复杂同步逻辑（oneshot/broadcast/Mutex）抽象为独立的 `ProfileWorker` 状态机，消除脆弱的 `drop(lock)` 模式，提升稳定性。
- [x] **增强配置寻址算子 (path_parser)**: 支持数组下标访问（如 `search.plugins.0.name`）及键值寻址（`a.key:val.b`），彻底消除配置数组（如插件列表）时需手动输入完整 JSON 的痛点。
- [x] **存储层深度清理 (FileStore v3)**: 
    - 修复 `delete_dlq_by_id` 等方法中的 6 层“毁灭金字塔”嵌套，改用迭代器或扁平化逻辑。
    - 将 `FileStore` 与 `MonolithicSealStore` 拆分为独立文件。
    - 统一 Token/Ticket 的领域映射逻辑，消除 40+ 个 Trait 方法中的重复 JSON 序列化模板。
- [x] **ConfigManager 内部重构**: 
    - 消除 `auto_migrate` 等函数中的深层嵌套逻辑（虽然 v0.3.3 已优化，但仍有扁平化空间）。
    - 将存储/缓存类型的元数据逻辑（是否分布式、默认 URL 等）从硬编码判断迁移至策略模式（Completed via `ConfigStrategy`).
- [x] **清理硬编码默认值**:
    *   **现状**: `BUILTIN_CLIENT_ID` 和 `DEF_MARKET_URL` 等关键默认值目前在代码中硬编码。
    *   **修改建议**: 必须通过构建脚本（`build.rs` 或 `Makefile`）在构建时注入。同步排查全工程中其他硬编码的默认值，确保配置的一致性。

### 🔵 P2: 中低优先级 (Medium/Low Priority)
- [x] **SQL 迁移抽象 (DSL/Trait)**: 为各 SQL Driver 提取通用的 `SchemaMigration` Trait，消除 SQLite/MySQL/Postgres 之间重复的列检查逻辑，并引入 `Transaction` 确保变更原子性。
- [x] **Doctor 插件化重构**: 将 `doctor.rs` 的过程化检测重构为基于 `DiagnosticTask` 插件的并发检测模型，提升扩展性。
- [x] **解耦进程编排逻辑**: 将 `cowen-server/src/cmd/mod.rs` 中复杂的进程监控、PID 管理和僵尸进程探测逻辑提取到独立的 `cowen-daemon` 编排组件中。
- [x] **提取独立诊断模块**: 将散落在各处的 `status.rs` 和 `audit.rs` 整合为独立的 `cowen-telemetry` (cowen-monitor) 模块。
- [x] **SSRF 防御与分级校验**: 为 `forwarder.rs` 增加 Webhook 转发白名单配置，并通过 `ssrf.rs` 实现分级校验。
- [x] **补全 OCP 抽象**: 将系统重置 (System Reset) 逻辑彻底模块化。
- [x] **高耦合度模块解耦：应用事件总线剪除 `cowen-monitor` 编译期强物理依赖** (v0.3.5)
    *   **落地成果**:
        1. **数据模型与客户端下沉**：状态核心指标模型及 `MonitorClient` 物理迁至 `cowen-common` 底层库，完成通信隔离。
        2. **高性能事件流**：引入 `GlobalEvent::Telemetry` 与 `ProxyRequestReceived` 事件，使用基于 Tokio `broadcast` 异步无锁进程内总线完成打点流动。
        3. **编译期物理剪除**：在 `cowen-auth` 与 `cowen-server` 中彻底物理剥离了对 `cowen-monitor` 的编译依赖。
        4. **完全兼容与平滑自愈**：上层 `MonitorServer::start` 通过后台协程静默捕获总线遥测流录入 SQLite，实现 0 既有测试改动的 100% 成功回归验证。
- [x] **高耦合度模块解耦：基于 Trait 隔离剪断对 `cowen-store` 的直接依赖** (v0.3.5)
    *   **落地成果**:
        1. **契约编程与依赖重塑**：确认 `cowen-auth` 生产核心逻辑面向抽象接口编写，唯一依赖的 `ConfigValidator` 验证器契约原生属于 `cowen-config` 共享层。
        2. **编译物理隔离**：更新 `ConfigValidator` 导入并彻底移除了对 `cowen-store` 编译期强生产依赖。
        3. **测试依赖平滑降级**：通过移入 `[dev-dependencies]` 作为测试依赖，确保本地单元/集成桩测试在 0 修改的情况下 100% 成功回归。

## 📐 P3: 长期架构演进与解耦计划 (Long-term Architecture Decoupling)

- [ ] **中耦合度模块解耦：通用守护进程化重构 `cowen-daemon`**
    *   **实现建议**: 抽象并提取子进程的通用生命周期控制 Trait（如 `start`, `stop` 等），使编排组件不再与特定 Worker 实现强绑定。
- [ ] **中耦合度模块解耦：将 `cowen-doctor` 演进为纯“测试套件执行器”**
    *   **实现建议**: 允许每个子领域模块（存储、配置、AI 插件等）自行在其内部编写及导出特定的诊断任务，医生模块仅负责并发拉起与聚合 Prettytable 打印。

---

## 📂 附件 (Attachments)

- 📄 **架构分析与核心源码审计报告**：包含 12 个 Crate 层级耦合度精细审计、源码精读、SOLID 原则落地以及具体解耦建议。
    - [ARCHITECTURE_AUDIT_REPORT.md](ARCHITECTURE_AUDIT_REPORT.md)

