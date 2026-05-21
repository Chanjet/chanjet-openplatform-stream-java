# cli/cowen v0.3.2 全栈执行级 WBS (Master Blueprint)

## Phase 1: 基础设施层 - 增强型配置引擎 (Config Engine)
**核心目标**：实现全路径配置管理，支持嵌套路径及跨文件（App/Profile）自动分发，彻底消除 YAML 手动编辑需求。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1.1** | **嵌套路径解析算子** | [x] | 在 `cowen-config` 实现 `path_parser.rs`，基于 `serde_json::Value` 处理点分路径写入与读取。 | `path_parser.rs` 及单元测试 | 1. [兼容] 不改变既有 YAML 缩进风格。<br>2. [验证] 单元测试覆盖深层嵌套（>3层）及类型转换。 |
| **1.2** | **ConfigManager 分发逻辑** | [x] | 修改 `config_manager.rs`，根据路径前缀（如 `storage.`）自动识别读写 `app.yaml` 还是 `profile.yaml`。 | `ConfigManager::set_value` / `get_value` | 1. [兼容] 支持单文件旧版配置平滑加载。<br>2. [E2E] `config set storage.store local` 后，验证 `app.yaml` 物理更新。 |
| **1.3** | **配置校验拦截器** | [x] | 实现 `ConfigInterceptor` SPI，增加端口范围 (1024-65535) 和 URL 格式预检。 | `interceptors.rs` | 1. [E2E] 设置非法端口时，CLI 必须报错并阻止写入，返回非零状态码。 |
| **1.4** | **数据脱敏与列表化** | [x] | 实现配置扁平化输出及敏感字段（secret/key/password）的 Masking 过滤器。 | `ConfigManager::list_values` | 1. [E2E] `config list` 输出中，`db_url` 密码部分和 `app_secret` 显示为 `******`。 |
| **1.5** | **CLI 配置命令集成** | [x] | 扩展 `Commands::Config`，实现 `set`, `get`, `list` 三个子命令。 | `cli/cowen/src/cmd/system.rs` 更新 | 1. [E2E] 验证 `cowen config get log.level` 能准确读取当前 Profile 状态。 |

## Phase 2: 核心架构层 - 单进程守护模型 (Single-Process Daemon)
**核心目标**：从“一环境一进程”迁移至“单进程多任务”架构，降低资源占用并提升管理效率。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **2.1** | **Worker 任务管理器** | [x] | 实现 `WorkerManager`，使用 `tokio::task::JoinSet` 和 `CancellationToken` 管理各 Profile 的独立协程。 | `cowen-server/src/daemon/manager.rs` | 1. [兼容] `--profile` 参数逻辑保持一致。<br>2. [E2E] 启动 3 个 Profile，`ps` 仅显示一个 PID，但各端口均可访问。 |
| **2.2** | **任务隔离与 Panic 自愈** | [x] | 包装 Worker 核心循环至 `AssertUnwindSafe`，实现简单的重启 Watchdog。 | `manager.rs` 容错逻辑 | 1. [验证] 人为注入 Panic，验证主进程不崩溃且受损 Worker 在 5s 内自动恢复。 |
| **2.3** | **Daemon 启动命令重构** | [x] | 修改 `daemon start` 逻辑：若检测到已有主进程运行，则转为 IPC 指令告知其启动新 Worker。 | `cli/cowen/src/cmd/daemon.rs` 更新 | 1. [E2E] 连续执行两次 `start` (不同 profile)，验证后一个成功并入前一个进程。 |

## Phase 3: 稳定性增强 - 优雅关机系统 (Graceful Shutdown)
**核心目标**：确保进程退出时，存量任务（Webhook 重试、Token 刷新）安全完成，存储连接平滑关闭。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **3.1** | **异步任务追踪器** | [x] | 实现 `ShutdownGate` (基于原子计数或 Semaphore)，追踪所有活跃的转发任务。 | `cowen-server/src/utils/shutdown.rs` | 1. [验证] 模拟高频转发，发送 SIGTERM 后，日志显示“Waiting for X tasks”并最终安全退出。 |
| **3.2** | **两阶段停机协议** | [x] | 改造 `bridge.rs`：第一阶段停止接收新事件，第二阶段执行存量 Draining（最大 10s）。 | `bridge.rs` 关机逻辑 | 1. [兼容] 确保跨平台（Unix/Windows）信号处理一致性。 |
| **3.3** | **存储层安全回收** | [x] | 在 `Vault` 和 `DlqStore` 实现异步 `shutdown` 挂钩，显式关闭 SQLx/Redis 连接池。 | 各存储实现类的 `shutdown()` | 1. [E2E] 关机后立即再次启动，不应出现 `database is locked` 或连接池耗尽错误。 |

## Phase 4: 交互与通信层 - IPC 授权同步 (Reliable Auth Sync)
**核心目标**：利用单进程优势，通过管理 API 实现 `init` 过程的秒级反馈与可视化进度条。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **4.1** | **授权管理端点** | [x] | 在 `cowen-monitor` 增加 `POST /v1/mgmt/auth/finalize` 和进度查询 API。 | `cowen-monitor/src/api/mgmt.rs` | 1. [验证] API 必须经过 Loopback 绑定校验，防止外部非法推送 Code。 |
| **4.2** | **Init 流程重构** | [x] | 改造 `cowen init`：不再轮询日志，改为 HTTP API 与正在运行的 Daemon 通信。 | `cli/cowen/src/cmd/auth.rs` 更新 | 1. [兼容] 支持无 Daemon 状态下的独立同步回退模式。<br>2. [E2E] 模拟 OAuth 完整路径，进度条显示正常。 |
| **4.3** | **交互式进度反馈** | [x] | 集成 `indicatif` 库，在终端渲染多阶段进度条。 | `init` 命令 UI 增强 | 1. [E2E] 错误发生时（如 Token 置换失败），终端立即红字显示原始报错信息。 |

## Phase 5: 性能优化层 - DLQ 存储演进 (DLQ Optimization)
**核心目标**：解决积压场景下的内存压力，实现分页与精确重试。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **5.1** | **存储接口分页增强** | [x] | 扩展 `DlqStore` SPI，新增 `get_by_id(id)` 和 `list_paged(offset, limit)`。 | `cowen-common/src/vault/dlq.rs` | 1. [兼容] 保持 `list_all` 接口存在以兼容旧脚本。<br>2. [验证] 数据库查询语句确认使用 Index。 |
| **5.2** | **重试逻辑内存调优** | [x] | 修改 `Forwarder::retry_message`：仅根据 ID 加载单条消息，不再全量列举。 | `forwarder.rs` 重构 | 1. [E2E] 在 DLQ 积压 5000+ 时，运行 `dlq retry <ID>` 内存增长接近 zero。 |
| **5.3** | **CLI 分页列表展示** | [x] | `dlq list` 增加分页参数，默认仅展示最新 20 条。 | `dlq.rs` 更新 | 1. [E2E] `dlq list --page 2` 能够准确输出第 21-40 条死信内容。 |

## Phase 6: 结项与回归 (Finalization)
**核心目标**：完成配置迁移，确保全量回归通过。

| ID | 任务名称 | 状态 | 实现细节 | 交付产物 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **6.1** | **自动迁移工具** | [x] | 在 `ConfigManager` 实现 `auto_migrate()`，将 Profile 中的全局项提取至 `app.yaml`。 | 迁移逻辑代码 | 1. [兼容] 迁移后必须备份原 YAML 文件。<br>2. [E2E] 升级后 `cowen status` 验证各项参数继承正确。 |
| **6.2** | **全量回归测试** | [x] | 执行全量 E2E 测试集（Case 1-53），包含单进程稳定性压力测试。 | 测试报告 | 1. [验证] `scripts/run_all_tests.sh` 通过率 100%。 |
