# cli/cowen v0.3.3 产品需求文档 (PRD)

> **状态**: `REVIEW`
> **版本**: v0.3.3
> **日期**: 2026-05-21

## 1. 版本背景与目标 (Context & Objectives)
在 v0.3.2 成功实现了单进程架构和优雅关机后，系统的外部行为已趋于稳定。然而，内部实现仍存在多处**复杂性陷阱**（如 Worker 管理的脆弱同步、ConfigManager 的深层嵌套、FileStore 的代码冗余）。

v0.3.3 的核心目标是**内部治理**：通过引入状态机模型、增强配置引擎、以及深度重构存储层，将系统的内部复杂度降低 30% 以上，并彻底消除配置数组时的交互痛点。

## 2. 核心功能需求 (Core Requirements)

### 2.1 Worker 生命周期状态机 (Worker Lifecycle State Machine)
*   **需求背景**: `WorkerManager` 目前同步逻辑脆弱。v0.3.3 需要引入状态机以支持更复杂的容错策略。
*   **功能描述**: 
    *   **状态收敛**: 抽象 `ProfileWorker` 状态机，包含 `Created`, `Starting`, `Running`, `Backoff`, `Failed`, `Draining`, `Stopped`。
    *   **退避重试 (Backoff)**: 当 Worker 意外崩溃时，进入 `Backoff` 状态。采用**指数退避算法**（如 1s, 2s, 4s... 最大 60s）。
    *   **熔断机制 (Circuit Breaker)**: 若 Worker 在 5 分钟内连续失败超过 5 次，状态标记为 `Failed` 并触发熔断。
    *   **手动恢复策略**: 处于 `Failed` 状态的 Worker **严禁自动重启**。用户必须通过显式的 `cowen daemon restart --profile X` 指令进行手动干预。
    *   **可观测性增强**: `cowen status` 实时显示 `Backoff` 倒计时或 `Failed` 熔断标记。
*   **关键技术确认**:
    *   **实现方式**: **手写状态机** (Handwritten State Machine)。不引入第三方 crate，以保证异步转移过程中的零成本抽象及对 `JoinHandle` 生命周期的绝对控制。

### 2.2 增强型配置引擎 - 配置自治与键值寻址 (Enhanced Config Engine)
*   **需求背景**: 彻底消除配置数组时的交互痛点。通过物理坍缩（自动重排）实现下标对用户的透明化。
*   **功能描述**: 
    *   **键值寻址 (Identifier Locator)**: 支持 `array.key:value.field` 定位。
    *   **即时绑定原则**: 身份定位器是即时生效的。若通过 `set name:A.name "B"` 修改了标识符，则原有的 `name:A` 路径立即失效。
    *   **严格边界检查**: 设置不存在的索引或 Key 时报错。
    *   **追加模式**: 支持 `+` 占位符。
    *   **删除与物理坍缩**: `unset` 操作后数组自动重排，索引自治。

### 2.3 存储层深度清理与平滑迁移 (Storage Cleanup & Migration)
*   **需求背景**: 解决代码嵌套与数据孤岛问题。
*   **功能描述**: 
    *   **路径标准化**: 存储路径重构为 `vault/{profile}/{prefix}/{id}.json`。
    *   **自动迁移**: 实现 `migrate_v2_to_v3` 逻辑，在 v0.3.3 首次启动时自动迁移 v0.3.2 布局。
    *   **领域模型统一**: 提取通用的存储映射 Trait。
    *   **GC 识别能力**: 存储层具备识别“孤儿文件”的能力，为诊断工具提供支撑。
*   **关键技术确认**:
    *   **触发时机**: **系统启动时静默触发** (Silent Migration on Boot)。迁移是底层架构变更的必要条件，不提供延迟迁移选项。

## 3. 影响范围评估 (Impact Assessment)

### 3.1 内部模块影响
*   **cowen-monitor**: 
    *   API 契约变更：`/v1/status` 将增加 `status_detail` 字段，包含重试次数和下次重试时间戳。
    *   数据结构：`StatusEntry` 将引入 `Metadata` 映射用于存储状态机元数据。
*   **cowen-server**: 
    *   `WorkerManager` 核心逻辑将被 `ProfileWorker` 状态机完全替换。
    *   信号处理函数将对接新的 `Draining` 状态。
*   **cowen-store**: 
    *   `FileStore` 物理布局发生不可逆变更，需配合迁移器执行。

### 3.2 外部工具与交互影响
*   **cowen-doctor**: 
    *   新增诊断项：支持“存储碎片检测”，利用新的 GC 接口识别孤儿文件。
    *   更新修复建议：当检测到 `Failed` 状态的 Worker 时，提示执行 `restart` 而非重新 `init`。
*   **CLI UX**: 
    *   `config` 子命令新增 `unset`。
    *   `status` 输出增强。

## 4. 非功能性需求 (NFRs)
*   **代码质量**: 必须通过 TDD 驱动重构。
*   **稳定性**: 引入熔断器后，系统在极端不稳环境下的 CPU/IO 震荡应降低 80% 以上。
*   **安全性**: 保持加密一致性，配置 `list` 时必须脱敏。

## 5. 任务计划 (High-level WBS)
1. **P1.1**: 实现 `ProfileWorker` 状态机原型（含 Backoff/CircuitBreaker）。
2. **P1.2**: 增强 `path_parser` 支持数组下标、追加与删除。
3. **P1.3**: 实现 `FileStore` 路径标准化与 `v2_to_v3` 迁移器。
4. **P1.4**: 归一化存储模型并完成回归验证。
