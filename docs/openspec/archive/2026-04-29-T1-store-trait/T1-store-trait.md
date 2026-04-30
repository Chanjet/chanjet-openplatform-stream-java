# OpenSpec 提案 - [Task-T1] 定义 Store Trait 及基础模型

## 1. 提案摘要
定义 Cowen v0.3.0 分布式存储层的统一接口 `Store` 以及核心数据模型 `Item`。

## 2. 变更范围
- [NEW] `cli/cowen/src/core/store/mod.rs`
- [MODIFY] `cli/cowen/src/core/mod.rs`

## 3. 技术设计
- **LLD 锚点**: [Store 抽象契约](../../lld/sections/02-contracts.md#CONTRACT_STORE)
- **Trait 定义**: 包含 `get`, `set`, `delete` 异步方法。
- **数据模型**: `Item` 结构体包含 `profile`, `key`, `value`, `updated_at`。

## 4. 验证计划
- **单元测试**: `test_store_interface` 验证 Trait 签名。
- **编译检查**: `cargo check` 确保异步 Trait 导出正确。

## 5. 归档状态
- **状态**: `DONE`
- **执行人**: Master Orchestrator
- **完成日期**: 2026-04-29
