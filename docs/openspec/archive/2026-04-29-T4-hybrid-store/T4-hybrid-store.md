# OpenSpec 提案 - [Task-T4] 实现 HybridStore 混合读写逻辑

## 1. 提案摘要
实现 `HybridStore`，将 Redis 作为高性能 Cache 层，SQL 作为持久化层，实现“权威持久化 + 缓存加速”的混合存储模式。

## 2. 变更范围
- [NEW] `cli/cowen/src/core/store/hybrid.rs`: 混合存储逻辑实现。
- [MODIFY] `cli/cowen/src/core/store/mod.rs`: 导出 `hybrid` 模块。

## 3. 技术设计
- **逻辑策略**:
  - `set`: Write-Through。先写 DB，成功后同步写 Redis。
  - `get`: Cache-Aside。先读 Redis，不中则读 DB 并回填 Redis。
- **LLD 锚点**: [HybridStore 编排契约](../../lld/sections/02-contracts.md#CONTRACT_HYBRID)
- **自愈机制**: Redis 异常时不阻塞 DB 操作。

## 4. 验证计划 (TDD)
- **单元测试**: `test_hybrid_get_hit`, `test_hybrid_get_miss_and_fill`, `test_hybrid_set_sync`。

## 5. 归档状态
- **状态**: `IN_PROGRESS`
- **执行人**: Master Orchestrator
- **开始日期**: 2026-04-29
