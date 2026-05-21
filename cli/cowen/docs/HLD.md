# cli/cowen v0.3.3 概要设计 (HLD)

> **版本**: v0.3.3
> **阶段**: Architecture Blueprint
> **状态**: `DRAFT`

## 1. 架构演进目标 (Architectural Evolution)
v0.3.3 的核心架构演进在于从“过程化控制”向“声明式/状态驱动控制”转变。重点通过状态机模式隔离复杂的异步副作用，并通过更精细的 I/O 层重构消除技术债。

## 2. 核心模块设计 (Core Module Design)

### 2.1 ProfileWorker 状态机模型
`WorkerManager` 将不再直接管理 `tokio::task::JoinHandle`，而是管理一组 `ProfileWorker` 容器。

*   **状态定义 (State Enum)**:
    *   `Created`, `Starting`, `Running`.
    *   `Backoff`: 崩溃后的等待期。
    *   `Failed`: 触发熔断。从 `Failed` 恢复至 `Starting` **必须由用户通过 `daemon restart` 命令显式触发**，不支持任何形式的隐式或自动唤醒。
    *   `Draining`, `Stopped`.

*   **状态可见性 (Observability)**:
    *   Monitor API (`/v1/status`) 扩展返回 `Backoff` 详情及 `Failed` 熔断原因。

### 2.2 路径解析器 (Path Parser) 扩展
`path_parser.rs` 支持数组 AST 解析、键值匹配寻址与坍缩删除。

*   **算子优先级**:
    1.  `field` (点号): 访问属性。
    2.  `locator` (`key:val`): 数组内部匹配寻址。
    3.  `index` (数字): 基础下标寻址。
    4.  `append` (`+`): 数组末尾新增。

*   **即时绑定机制**:
    寻址引擎基于当前内存中的配置快照进行匹配。若一次 `set` 操作改变了对象的标识符字段，该定位器在下一条指令中将立即失效。

### 2.3 存储层 (Storage Layer) 扁平化与迁移
消除 `FileStore` 中的深层嵌套与布局混乱。

*   **物理布局标准化**:
    标准路径：`vault/{profile}/{prefix}/{id}.json`。

*   **平滑迁移器 (Migration Module)**:
    启动时执行单向静默迁移。

*   **垃圾回收接口 (GC Interface)**:
    定义 `list_orphans()` 方法，通过扫描物理目录并对比当前配置（如插件列表），识别出不再被配置引用的“孤儿数据文件”，为诊断工具提供数据。


## 3. 容错与安全性 (Fault Tolerance & Security)
*   **熔断保护**: 防止因环境故障导致的死循环重启。
*   **配置脱敏**: 确保 `unset` 敏感项后不再残留痕迹。
