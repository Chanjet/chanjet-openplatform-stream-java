# cli/cowen v0.3.4 全栈执行级 WBS (Master Blueprint)

## Phase 2: 工程卓越与自动化专项 (Engineering Excellence & Automation)
**核心目标**：实现守护进程独立化与诊断持久化，通过策略模式消除硬编码，提升系统安全防御等级。

### 工作包 1: 核心剥离与跨进程底座 (Core Decoupling & IPC Foundation)
| ID | 任务名称 | 目标与方法 | 关键产物 | 估时 (MD) | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :---: | :--- |
| **2.1** | **独立守护进程构建** | **方法**: 提取编排逻辑至 `cowen-daemon` 二进制。 | `cowen-daemon` bin | 1.5 | [验证] 独立二进制可正常拉起 Worker 并监听 UDS。 |
| **2.2** | **UDS IPC 与自动拉起** | **方法**: 实现 UDS 通信协议与 **RETRY_FAST (5次/2s)** 连接算法。 | `ipc` 模块 | 1.0 | [TDD] 模拟 Daemon 未启动，CLI 成功触发冷拉起并同步状态。 |

### 工作包 2: 存储增强与诊断持久化 (Storage & Persisted Observability)
| ID | 任务名称 | 目标与方法 | 关键产物 | 估时 (MD) | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :---: | :--- |
| **2.3** | **SQL 迁移抽象 (DSL)** | **方法**: 定义 `SchemaMigration` Trait，消除多后端 DDL 冗余。 | `migration_trait.rs` | 0.5 | [验证] SQLite/MySQL 变更脚本实现 1:1 核销。 |
| **2.4** | **诊断持久化与 GC** | **方法**: 集成 SQLite，实现 **15天/1万条** 滚动清理算法。 | `telemetry.db` | 1.0 | [TDD] 模拟海量数据，验证物理删除逻辑的准确性。 |
| **2.5** | **Events 指令实现** | **方法**: 实现 `cowen events`，支持历史故障轨迹查询。 | `events` 命令 | 0.5 | [验证] 成功展示 Worker 的 Backoff 历史与 Panic 详情。 |

### 工作包 3: 架构治理与安全加固 (Governance & Security)
| ID | 任务名称 | 目标与方法 | 关键产物 | 估时 (MD) | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :---: | :--- |
| **2.6** | **Config 策略模式重构** | **方法**: 引入 `ConfigStrategy` SPI，解耦后端元数据逻辑。 | `strategy.rs` | 1.0 | [重构] 核心模块行数减少 30% 以上，消除硬编码。 |
| **2.7** | **SSRF 三级防护实现** | **方法**: 实现 `Strict/Flexible` 等级校验，支持 CIDR 匹配。 | `ssrf.rs` | 0.5 | [安全] 成功拦截 Flexible 模式外的非法转发。 |
| **2.8** | **Doctor 插件化与并行** | **方法**: 重构为基于 `inventory` 静态注册的并发检测模型。 | `doctor/task.rs` | 1.0 | [架构] 新增检测项无需修改主流程，支持并发执行。 |

### 工作包 4: 工程自动化收尾 (Engineering Closure)
| ID | 任务名称 | 目标与方法 | 关键产物 | 估时 (MD) | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :---: | :--- |
| **2.9** | **Makefile 模块化** | **方法**: 拆分主 Makefile 为功能脚本，清理敏感 ID。 | `scripts/` | 0.2 | [维护] Makefile 行数降低 50% 以上。 |
| **2.10** | **全量回归验证** | **方法**: 执行 54 组 E2E 回归测试。 | 验证报告 | 0.3 | [验证] Case 01-54 100% 通过。 |

**总工时估算**: 7.5 人日 (Man-Days)
