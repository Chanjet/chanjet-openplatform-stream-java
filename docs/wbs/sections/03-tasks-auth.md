# 业务编排层开发任务 (Auth & Hybrid Tasks)

## [Task-T4] 实现 HybridStore 混合读写逻辑
- **LLD 强映射锚点**: [HybridStore 编排契约](../../lld/sections/02-contracts.md#CONTRACT_HYBRID)
- **任务描述**: 实现 `HybridStore` 结构体，组合 Cache 和 Persistence 两个 Store 实例，并按照 [MLDT](../../lld/sections/04-logic-ops.md#MLDT_HYBRID) 实现读写策略。
- **验收标准 (DoD)**:
  - 模拟 Cache 失效时，能成功回源 Persistence 层获取数据并回填 Cache。
  - 持久化失败时，不更新缓存。
- **TDD 物理样本**:
  - `get` 场景：Cache 返回 `None` -> DB 返回 `"val"` -> 最终返回 `"val"` 且 Cache 被更新。

## [Task-T6] 商店应用长效凭据自愈实现
- **LLD 强映射锚点**: [商店应用令牌自愈算子](../../lld/sections/04-logic-ops.md#MLDT_STORE_AUTH)
- **任务描述**: 
  - 在 `OAuth2Provider` 中增加对 `user_permanent_code` / `org_permanent_code` 的处理逻辑。
  - 实现当 `refresh_token` 失效时，自动触发永久授权码换票的闭环流程。
- **验收标准 (DoD)**:
  - 模拟 `refresh_token` 过期且 DB 存在永久码，系统能自动恢复 Token 访问。
  - 成功恢复后，DB 中的新版 `access_token` 和 `refresh_token` 均被正确持久化。
- **TDD 物理样本**:
  - `MockRefresh` 返回 4007 -> `Store.get(permanent_code)` 成功 -> 调用换票接口成功 -> 最终业务请求成功重试。

---

## [Task-T5] 适配 Auth 模块至分布式存储
- **LLD 强映射锚点**: [静态依赖视图](../../lld/sections/01-structure.md)
- **任务描述**: 重构 `AuthService` 和 `Vault`，使其接受 `Arc<dyn Store>` 注入，移除对本地文件的硬编码依赖。
- **验收标准 (DoD)**:
  - Token 刷新后，数据正确写入共享数据库。
  - 多实例启动后可共享同一个 Token 状态。

---
*关联 PRD：[混合存储能力](../../prd/sections/03-feature-list.md#Feature-05)*
