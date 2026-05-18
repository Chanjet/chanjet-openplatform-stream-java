# cli/cowen v0.3.1 任务拆解结构 (WBS)

基于 v0.3.1 强调**物理 Crate 隔离**的 PRD、HLD 和 LLD，我们将开发任务分解为以下有序的阶段 (Phases) 和具体的开发任务 (Tasks)。所有开发将严格遵循 TDD（测试驱动开发）流程。

## 🎯 WBS 准入与完成条件 (Completion & Acceptance Criteria)
要被正式判定为阶段性或全局“已完成”，开发必须满足以下极其严格的验收红线：
1.  **所有既有测试用例 100% 通过**：任何阶段的开发均严禁引起原有功能受损，必须在本地及持续集成（CI）环境中顺利通过原有全部单元测试、集成测试与 E2E 场景用例。
2.  **工作区完美零警告编译**：全工作区（`cargo build --workspace`）必须通过无警告（`warnings`）和无错误的干净编译。
3.  **TDD 单元测试全覆盖**：所有新增功能代码均必须伴随 100% 通过的 TDD 单元测试用例，严禁在没有对应失败测试用例的情况下编写生产代码。
4.  **新增 E2E 场景闭环验证**：各个阶段新增的自动化 E2E 场景验证脚本（如配置热载、监控 API、自检工具、插件加载、Panic自愈）均必须无异常通过并归档。
5.  **原有测试资产零变更兼容**：重构和新特性必须实现 100% 向后兼容，所有原有既有测试用例（如 E2E 场景 `case_1` ~ `case_44` 等）必须保持 **100% 原样无须做任何代码级修改即可顺利全绿通过**。严禁修改或删除原有测试以掩盖生产代码的非预期不兼容变更。

---

## Phase 1: 基础设施准备与 Crate 脚手架 (Infrastructure & Scaffolding)
*   **Task 1.1**: 更新工作区 `Cargo.toml`，注册新的 Crate 成员。
*   **Task 1.2**: 使用 `cargo new --lib` 创建以下隔离的内部 Crate：
    *   `crates/cowen-infra` (新底座工具包)
    *   `crates/cowen-config`
    *   `crates/cowen-monitor`
    *   `crates/cowen-doctor`
    *   `crates/cowen-search`
    *   `crates/cowen-search-embedding` (剥离现有的 `cowen-ai`)
*   **验收标准 (Acceptance Criteria)**:
    1. 工作区 `Cargo.toml` 正确注册 6 个新 Crate，并在物理目录结构中正确生成。
    2. 执行 `cargo check --workspace` 编译无报错。
    3. 全局既有测试用例 100% 顺利通过。

## Phase 2: 核心依赖去上帝化重构 (Decoupling & Splitting cowen-common)
*   **Task 2.1: `cowen-infra` 底座创建与工具沉降**
    *   在 `crates/cowen-infra` 中编写基础工具代码。
    *   将原 `cowen-common` 中通用的加盐混淆 (obfs)、系统物理路径计算、低级时间转换等业务无关工具逻辑全部沉降至此 Crate。
*   **Task 2.2: `cowen-common` 瘦身与极净化**
    *   清理 `cowen-common` 中的 `Cargo.toml` 依赖，移除 reqwest、tokio、redis 等一切厚重 I/O 库绑定。
    *   瘦身其源码，使其仅包含最通用稳定的核心数据模型与 SPI 契约 Trait。
*   **Task 2.3: 全局依赖树梳理与编译修复**
    *   重构 `cowen-server`、`cowen-auth`、`cowen-store` 的 `Cargo.toml` 声明和源码中模块路径引入。
    *   确保依赖层次重定向后全工程一次性编译通过，物理上隔离循环依赖风险。
*   **验收标准 (Acceptance Criteria)**:
    1. `cowen-common` 的 `Cargo.toml` 不含有任何网络及存储等重依赖，仅依赖极其稳定的核心模型库。
    2. 依赖 `cowen-common` 的各个业务 Crate（如 `cowen-server` 等）重新定位底层结构引用，编译无警告和报错。
    3. 运行 `cargo test --workspace` 完美全绿，原有所有测试用例 100% 通过。

## Phase 3: 配置热重载 (cowen-config)
*   **Task 3.1: `cowen-config` 核心实现**
    *   引入 `notify` 和 `tokio` 依赖。
    *   实现基于 `notify` 的配置文件变更监听器和 `SIGHUP` 信号监听。
    *   利用 `tokio::sync::watch` 对外暴露配置订阅能力。
*   **Task 3.2: Daemon 集成与日志级别动态调整**
    *   在 `cowen-server` (Daemon) 中引入 `cowen-config`。
    *   修改 `ProxyServer` 等长连接后台任务，使其订阅配置更新。
    *   对接 `tracing-subscriber` 的 Reload 句柄，随配置更新动态改变日志级别。
*   **Task 3.3 (E2E)**: 实现 `tests/e2e/scripts/case_45_config_hot_reload.sh` 验证热重载不断流及日志级别的变化。
*   **验收标准 (Acceptance Criteria)**:
    1. 新增针对不可热载字段（如 `db_url` 等物理模型关键值）变动拦截的 TDD 单元测试并 100% 通过。
    2. 自动化 E2E 脚本 `case_45_config_hot_reload.sh` 运行成功：修改 `app.yaml` 日志级别后进程不断流且 PID 保持不变，新的日志级别在 Daemon 运行中即时生效。

## Phase 4: 监控与健康 API (cowen-monitor)
*   **Task 4.1: `cowen-monitor` 服务搭建**
    *   引入 `prometheus` 和 `axum`。
    *   实现独立运行在本地端口的 HTTP 服务，暴露 `/health` 和 `/metrics`。
*   **Task 4.2: 宏定义与无侵入埋点**
    *   在 `cowen-monitor` 导出打点宏 (如 `counter!()`)。
    *   在 `cowen-server` (Proxy 处理层等) 引入宏进行无侵入式指标统计。
*   **Task 4.3 (E2E)**: 实现 `tests/e2e/scripts/case_46_metrics_health.sh` 验证端点连通性及指标累加逻辑。
*   **验收标准 (Acceptance Criteria)**:
    1. 新增针对监控注册中心和指标增加的 TDD 单元测试并 100% 通过。
    2. 自动化 E2E 脚本 `case_46_metrics_health.sh` 运行成功：监控端口独立运行在 `127.0.0.1`，通过请求代理转发后，`/metrics` 能准确采集累加指标，`/health` 输出规范的 UP 状态。

## Phase 5: 环境自检工具 (cowen-doctor)
*   **Task 5.1: `cowen-doctor` SPI 与调度台**
    *   定义 `Diagnostic` Trait 及 `DiagnosticResult` 模型。
    *   实现并发诊断调度引擎。
*   **Task 5.2: 诊断器实现与装配**
    *   在 `cowen` 主项目中，依赖底层网络/存储模块实现具体的网络探测 (`NetworkDiagnostic`) 和数据库探测 (`StorageDiagnostic`)。
    *   将这些探测器注册到 `cowen-doctor`。
*   **Task 5.3: CLI 命令集成**
    *   新增 `cowen system doctor` 命令行入口，格式化输出。
*   **Task 5.4 (E2E)**: 实现 `tests/e2e/scripts/case_47_system_doctor.sh` 验证故意配错时能输出预期的 ERROR 和建议。
*   **验收标准 (Acceptance Criteria)**:
    1. 新增诊断器调度超时的 TDD 单元测试并 100% 通过。
    2. 自动化 E2E 脚本 `case_47_system_doctor.sh` 运行成功：当手动损坏数据库配置或断开网络时，运行 `cowen system doctor` 能准确抛出 `[ERROR]` 并输出对应的推荐修复建议（`Recommendation`）。

## Phase 6: API 搜索插件化 (cowen-search)
*   **Task 6.1: `cowen-search` SPI 与内置实现**
    *   定义 `SearchProvider` Trait。
    *   实现内置基于字符串匹配的 `StringMatchProvider`。
*   **Task 6.2: `cowen-search-embedding` 动态库化**
    *   迁移 ONNX 模型逻辑，配置为 `cdylib` 编译目标。
    *   建立 C ABI 边界 (`v1_init`, `v1_free`)。
*   **Task 6.3: 动态加载机制与 Fallback**
    *   在 `cowen-search` 中引入 `libloading`，实现运行时加载动态库的安全逻辑。
    *   实现加载失败时的优雅降级 (Fallback) 逻辑。
    *   修改 `api list --search` 命令路由至 `cowen-search`。
*   **Task 6.4 (E2E)**: 实现 `tests/e2e/scripts/case_48_search_plugin.sh` 验证降级警告及插件正常加载的行为。
*   **验收标准 (Acceptance Criteria)**:
    1. 新增插件加载失败时降级机制的 TDD 单元测试并 100% 通过。
    2. 自动化 E2E 脚本 `case_48_search_plugin.sh` 运行成功：在移除动态链接库时，`api list --search` 命令能抛出优雅降级警告并成功退回到内置的字符串检索匹配模式；插件存在时能正常调用 C-ABI 动态载入推理。

## Phase 7: 健壮性与进程自愈增强 (Robustness & Self-Healing Enhancements)
*   **Task 7.1: DLQ 存储异常 Panic 防护实现**
    *   重构 `Forwarder::new` 签名，返回 `Result<Self, CowenError>`。
    *   在 `bridge.rs` (Daemon 启动) 和 `cmd/dlq.rs` (重试入口) 中捕获存储初始化异常，优雅输出日志并平滑退场，杜绝进程崩溃 Panic。
    *   编写单元测试模拟连接损坏场景进行验证。
*   **Task 7.2: 智能动态 Token 检查与自适应刷新实现**
    *   在 `renewer.rs` 和 `bridge.rs` 中重构硬编码的 10 分钟检测间隔，设计自适应计算延迟 `next_check = (expires_at - now) * 0.8`。
    *   实现 `[30s, 3600s]` 边界夹持保护及随机秒级抖动 Jitter 偏置，防止惊群效应。
    *   编写单元测试验证基于 Token 剩余寿命自适应计算的时间延迟与抖动的稳定性。
*   **Task 7.3 (E2E)**: 实现 `tests/e2e/scripts/case_49_robustness_check.sh` 自动验证死信队列损坏时的安全退出和 Token 定时调度的自适应演进状态。
*   **验收标准 (Acceptance Criteria)**:
    1. 新增死信初始化坏库测试用例、Token 定时休眠计算公式用例（含上限 3600s、下限 30s 与随机 Jitter 偏移）的 TDD 单元测试并 100% 全绿通过。
    2. 自动化 E2E 脚本 `case_49_robustness_check.sh` 运行成功：在坏库场景下进程平滑输出致命级日志并安全退出（退出码为 1），主进程不崩溃；在大周期测试下，Token 检测线程严格按照寿命公式及波动范围进行自适应休眠。

## Phase 8: 文档与发布 (Release & Docs)
*   **Task 8.1**: 更新 `docs/COMMANDS.md` 补充新命令和配置项。
*   **Task 8.2**: 更新安装脚本/Makefile，处理 `libcowen_search_embedding` 的可选分发。
*   **验收标准 (Acceptance Criteria)**:
    1. 编译构建生成的发布压缩包大小满足预定指标，且动态库打包逻辑可分离。
    2. 全工程及所有测试用例、E2E 测试脚本 100% 顺利通过。