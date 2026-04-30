# OpenSpec 提案 - [Task-T3] 实现 Redis 驱动

## 1. 提案摘要
使用 `redis` 库实现 `Store` trait，用于高性能缓存 Token 和分布式信号量。

## 2. 变更范围
- [NEW] `cli/cowen/src/core/store/redis.rs`: Redis 驱动实现。
- [MODIFY] `cli/cowen/src/core/store/mod.rs`: 导出 `redis_store` 模块。

## 3. 技术设计
- **依赖库**: `redis = { version = "0.27", features = ["tokio-comp", "aio"] }`
- **LLD 锚点**: [Store 抽象契约](../../lld/sections/02-contracts.md#CONTRACT_STORE)
- **Key 规则**: `profile:key` 格式。

## 4. 验证计划 (TDD)
- **单元测试**: `test_redis_store_lifecycle` 模拟 Redis 操作。

## 5. 归档状态
- **状态**: `IN_PROGRESS`
- **执行人**: Master Orchestrator
- **开始日期**: 2026-04-29
