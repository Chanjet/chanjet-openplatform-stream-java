# 模块契约与签名 (Module Contracts)

## 1. Store 抽象契约 (Store Abstraction) {#CONTRACT_STORE}
- **① 原子化签名**:
  - `get(profile: &str, key: &str) -> Result<String>`
  - `set(profile: &str, key: &str, value: &str) -> Result<()>`
  - `delete(profile: &str, key: &str) -> Result<()>`
- **② 实现能力与目标**: 提供统一的 K/V 存储接口，支持 500+ QPS 的高频读写。
- **③ 物理约束**: 
  - 实现类必须保证并发安全 (`Send + Sync`)。
  - 必须支持通过 `SqlStore` 映射到物理表，通过 `RedisStore` 映射到 Redis Key。
- **④ 物理对账**: `(Verified)([Cowen v0.2.x 存储快照](../../references/cowen-v02-snapshot.md))`
- **⑤ 逻辑蓝图链接**: `N/A (Standard Interface)`

## 2. HybridStore 编排契约 (Hybrid Orchestrator) {#CONTRACT_HYBRID}
- **① 原子化签名**:
  - `get(profile: &str, key: &str) -> Result<String>`
  - `set(profile: &str, key: &str, value: &str) -> Result<()>`
- **② 实现能力与目标**: 实现“缓存优先、异步/同步落库”的混合策略。
- **③ 物理约束**: 
  - 若 Cache 失败，必须能够穿透至 Persistence 层获取数据。
  - **[自愈防呆]**: 必须处理缓存过期与持久化数据不一致的边缘情况。
- **④ 物理对账**: `(Verified)([PRD 混合存储数据归类规则](../../prd/sections/04-business-rules.md#RULE_HYBRID_DATA))`
- **⑤ 逻辑蓝图链接**: `[Hybrid 读写算子](./04-logic-ops.md#MLDT_HYBRID)`

## 3. SqlStore 驱动契约 (SQL Driver) {#CONTRACT_SQL}
- **① 原子化签名**:
  - `init_pool(db_url: &str) -> Result<Pool>`
- **② 实现能力与目标**: 支持 MySQL, PG, SQLServer。
- **③ 物理约束**: 
  - 必须通过 `SqlDriver` 特性解耦不同数据库的语法差异。
  - **[自愈防呆]**：各 Driver 需自行处理连接超时并实现指数退避重连逻辑。
- **④ 物理对账**: `(Verified)([ADR-01 选型记录](../../hld/sections/03-adr.md#ADR-01))`
- **⑤ 逻辑蓝图链接**: `[SQL 执行与事务处理](./04-logic-ops.md#MLDT_SQL)`

---
*关联包划分：[静态依赖视图](./01-structure.md)*
