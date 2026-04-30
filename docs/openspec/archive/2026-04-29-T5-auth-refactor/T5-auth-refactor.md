# OpenSpec 提案 - [Task-T5] 适配 Auth 模块至分布式存储

## 1. 提案摘要
重构 `AuthService` 和 `Vault`，引入 `Store` 抽象，使其支持分布式 Token 存储与共享。

## 2. 变更范围
- [MODIFY] `cli/cowen/src/core/vault.rs`: 使用 `Store` 替换文件存储。
- [MODIFY] `cli/cowen/src/auth/mod.rs`: 注入 `Store` 依赖。

## 3. 技术设计
- **依赖注入**: `Vault` 结构体持有 `Arc<dyn Store>`。
- **兼容逻辑**: 保留对旧版本 `.vault` 文件的读取能力（作为迁移起点或回退方案）。
- **LLD 锚点**: [模块依赖图](../../lld/sections/04-modules.md)

## 4. 验证计划 (TDD)
- **单元测试**: `test_vault_with_mock_store` 验证 Token 存取。

## 5. 归档状态
- **状态**: `IN_PROGRESS`
- **执行人**: Master Orchestrator
- **开始日期**: 2026-04-29
