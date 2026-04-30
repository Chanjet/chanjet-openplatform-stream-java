# 存储层开发任务 (Storage Layer Tasks)

## [Task-T1] 定义 Store Trait 及基础模型
- **LLD 强映射锚点**: [Store 抽象契约](../../lld/sections/02-contracts.md#CONTRACT_STORE)
- **任务描述**: 在 `crate::core::store` 中定义 `Store` trait，并定义 `cowen_storage` 的 DTO 模型。
- **验收标准 (DoD)**:
  - 代码编译通过。
  - `Store` trait 包含 get/set/delete 方法。
- **TDD 物理样本**:
  - Input: `profile="dev", key="app_key", value="AK123"`
  - Output: `Ok(())`

## [Task-T2] 实现 SQLx 驱动 (MySQL/PG/SQL)
- **LLD 强映射锚点**: [SqlStore 驱动契约](../../lld/sections/02-contracts.md#CONTRACT_SQL)
- **任务描述**: 使用 `sqlx` 实现 `Store` trait，支持 MySQL、PostgreSQL 和 SQLServer。
- **验收标准 (DoD)**:
  - 成功连接到测试数据库。
  - 成功执行 `INSERT ... ON CONFLICT` 或等价操作。
  - 物理表 `cowen_storage` 字段与 [DDL](../../lld/sections/05-dto-schemas.md#DDL_STORAGE) 一致。

---
*关联 LLD：[物理模型萃取](../../lld/sections/05-dto-schemas.md)*
