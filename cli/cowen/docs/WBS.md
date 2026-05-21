# cli/cowen v0.3.4 全栈执行级 WBS (Master Blueprint)

## Phase 2: 架构解耦与工程卓越专项 (Decoupling & Excellence)
**核心目标**：实现守护进程独立化与诊断持久化，通过策略模式消除硬编码，提升系统安全防御等级。

| ID | 任务名称 | 目标与方法 | 是否兼容 | 关键产物 | 估时 (MD) | 质量门禁 |
| :--- | :--- | :--- | :---: | :--- | :---: | :--- |
| **2.1** | **独立守护进程构建** | **方法**: 提取编排逻辑，构建独立的 `cowen-daemon` 二进制产物。 | 是 | `cowen-daemon` binary | 2.0 | [验证] CLI 成功通过子进程调用新 daemon。 |
| **2.2** | **SSRF 安全等级实现** | **方法**: 实现 `Strict/Flexible` 等级校验，支持 CIDR 格式匹配。 | 是 | `security/ssrf.rs` | 1.0 | [安全] 成功拦截 Flexible 模式外的非法转发。 |
| **2.3** | **诊断数据持久化** | **方法**: 提取 `cowen-telemetry` 模块，集成 SQLite 存储历史状态。 | 是 | `telemetry.db`, 历史查看命令 | 1.5 | [验证] 重启后仍可查看历史 Backoff 记录。 |
| **2.4** | **ConfigManager 策略化** | **方法**: 引入 `ConfigStrategy` SPI，解耦后端元数据逻辑。 | 是 | `strategy.rs`, 重构版 `config_manager.rs` | 1.0 | [重构] 核心代码量减少，支持动态分发。 |
| **2.5** | **SQL 迁移抽象 (DSL)** | **方法**: 提取 `SchemaMigration` Trait，统一多数据库变更逻辑。 | 是 | `migration_trait.rs` | 0.5 | [验证] DDL 变更在各后端执行一致。 |
| **2.6** | **Doctor 插件化重构** | **方法**: 将检测项重构为基于并发插件的任务模型。 | 是 | `doctor/task.rs` | 1.0 | [架构] 支持第三方检测任务动态注入。 |
| **2.7** | **Makefile 模块化** | **方法**: 拆分主 Makefile 为功能脚本，清理敏感 ID。 | 是 | `scripts/build/`, `scripts/test/` | 0.5 | [维护] Makefile 逻辑清晰，无硬编码。 |

**总工时估算**: 7.5 人日 (Man-Days)
