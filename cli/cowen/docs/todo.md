# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🔴 P0: 紧急且关键 (Critical & Urgent)
- [x] 🎯 **消除潜在 Panic** (已规划，并入 v0.3.1 PRD 2.5): 修复 `forwarder.rs` 中 `DlqStore::new` 的 `unwrap` 调用。
- [x] 🎯 **动态 Token 检查策略** (已规划，并入 v0.3.1 PRD 2.6): 替换 `renewer.rs` 和 `bridge.rs` 中硬编码的 10 分钟轮询间隔。

## 🟠 P1: 高优先级 (High Priority)
- [x] **重构授权同步机制**: 现已实现基于 Monitor API (IPC) 的实时进度同步与进度条展示。
- [x] **实现优雅关机 (Graceful Shutdown)**: 显式跟踪所有异步任务，确保安全回收资源。
- [x] **优化 DLQ 重试逻辑**: 实现分页查询与按 ID 精确检索，解决了 OOM 风险。
- [x] **重构 Worker 生命周期管理**: 引入 `ProfileWorker` 状态机与指数退避算法，彻底消除了 `drop(lock)` 模式。
- [x] **增强配置寻址算子 (path_parser)**: 现已支持数组下标（`a.0.b`）、键值寻址（`a.key:val.b`）及追加模式（`+`），实现了配置交互自治。
- [x] **存储层深度清理 (FileStore v3)**: 
    - 物理拆分为 `core.rs`, `sealed.rs`, `migration.rs`。
    - 引入 `StoreItem` Trait，消除了 40+ 方法中的重复序列化模板。
    - 标准化为目录树结构，消除了 6 层以上的逻辑嵌套。
- [ ] **ConfigManager 内部重构**: 
    - 消除 `auto_migrate` 等函数中的深层嵌套逻辑（虽然 v0.3.3 已优化，但仍有扁平化空间）。
    - 将存储/缓存类型的元数据逻辑（是否分布式、默认 URL 等）从硬编码判断迁移至策略模式（Planned for Phase 2）。

## 🔵 P2: 中低优先级 (Medium/Low Priority)
- [ ] **SQL 迁移抽象 (DSL/Trait)**: 为各 SQL Driver 提取通用的 `SchemaMigration` Trait。
- [ ] **Doctor 插件化重构**: 将 `doctor.rs` 重构为基于 `DiagnosticTask` 插件的并发检测模型。
- [ ] **解耦进程编排逻辑**: 将复杂的进程监控、PID 管理提取到独立的 `cowen-daemon` 组件中。
- [ ] **提取独立诊断模块**: 整合 `status.rs` 和 `audit.rs` 为独立的 `cowen-telemetry` 模块。
- [ ] **灵活的 SSRF 防御**: 为 `forwarder.rs` 增加 Webhook 转发白名单配置。
- [ ] **构建脚本脱敏**: 移除 `Makefile` 中硬编码的 `OFFICIAL_APP_KEY`。
- [ ] **拆解 Makefile**: 将构建逻辑拆分为独立脚本。
- [ ] **容器化测试闭环**: 实现全量 E2E 在 Docker 内一键运行。
- [ ] **补全 OCP 抽象**: 将系统重置 (System Reset) 逻辑彻底模块化。
