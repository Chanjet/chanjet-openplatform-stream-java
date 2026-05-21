# cli/cowen v0.3.3 全栈执行级 WBS (Master Blueprint)

## Phase 1: 内部治理与架构收敛 (Internal Governance & Convergence)
**核心目标**：通过架构重构与存储层归一化，将系统内部复杂度降低 30%，并实现配置交互的完全自治。

| ID | 任务名称 | 变动范围与影响 | 关键产物 | 估时 (MD) | 依赖 | 质量门禁 (Quality Gates) |
| :--- | :--- | :--- | :--- | :---: | :---: | :--- |
| **1.1** | **ProfileWorker 状态机** | `cowen-server`, `cowen-monitor`。增强自愈确定性。 | `state.rs`, `manager.rs` 重构 | 2.5 | - | 1. [TDD] 9 条状态转移边覆盖 100%。<br>2. [重构] 移除所有显式的 `drop(lock)`。 |
| **1.2** | **Config 数组路径算子** | `cowen-config`。优化插件及多环境交互。 | 增强版 `path_parser.rs` | 1.5 | - | 1. [验证] 支持 `a.key:val.b` 及 `+` 语法。<br>2. [E2E] 通过 `unset` 验证数组坍缩。 |
| **1.3** | **FileStore 物理拆分** | `cowen-store`, `cowen-common`。提升 I/O 稳定性。 | `file.rs`, `migration.rs` | 1.5 | 1.4 | 1. [重构] 核心方法嵌套深度 ≤ 3 层。<br>2. [验证] 旧版数据自动在线迁移成功。 |
| **1.4** | **模型归一化** | `cowen-common`, `cowen-auth`。降低扩容成本。 | `store_trait.rs` | 1.0 | - | 1. [重构] 存储模板代码减少 40%。<br>2. [验证] 加解密一致性。 |
| **1.5** | **全量回归验证** | 全工程。确保治理后无功能退化。 | 测试报告, 新测例 | 1.0 | 1.1-1.4 | 1. [验证] Case 01-53 全量通过。<br>2. [验证] 模拟 5 分钟内 5 次 Panic 触发熔断。 |

**总工时估算**: 7.5 人日 (Man-Days)
