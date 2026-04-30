# OpenSpec 提案 - [Task-T6] 扩展 init 命令支持多后端

## 1. 提案摘要
扩展 `cowen init` 指令，支持通过 `--store`, `--db-url`, `--cache`, `--cache-url` 参数配置分布式存储后端。

## 2. 变更范围
- [MODIFY] `cli/cowen/src/main.rs`: 增加 CLI 参数定义，实现 `create_vault` 动态创建逻辑。
- [MODIFY] `cli/cowen/src/core/config.rs`: 增加 `StorageConfig` 结构。
- [MODIFY] `cli/cowen/src/cmd/init.rs`: 实现存储配置的持久化。

## 3. 技术设计
- **参数注入**: 在 `run()` 阶段预读 `init` 参数，确保注入到 `execute` 的 `vault` 实例已指向正确的后端。
*   **自愈性**: 默认使用 `local` 模式，确保老用户无感升级。

## 4. 验证计划 (TDD)
- **集成测试**: `cowen init --profile test-mysql --store mysql --db-url "..."` 验证配置是否正确存入 `.yaml`。

## 5. 归档状态
- **状态**: `DONE`
- **执行人**: Master Orchestrator
- **完成日期**: 2026-04-29
