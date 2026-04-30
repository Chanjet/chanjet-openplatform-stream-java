# OpenSpec 提案 - [Task-T2] 实现 SQLx 驱动

## 1. 提案摘要
使用 `sqlx` 实现 `Store` trait，支持 MySQL, PostgreSQL 和 SQLServer 的分布式共享存储。

## 2. 变更范围
- [MODIFY] `Cargo.toml`: 添加 `sqlx` 依赖。
- [NEW] `cli/cowen/src/core/store/sql.rs`: SQL 驱动实现。
- [MODIFY] `cli/cowen/src/core/store/mod.rs`: 导出 `sql` 模块。

## 3. 技术设计
- **依赖库**: `sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "mysql", "postgres", "mssql", "chrono", "json"] }`
- **LLD 锚点**: [SqlStore 驱动契约](../../lld/sections/02-contracts.md#CONTRACT_SQL)
- **并发控制**: 采用 `INSERT ... ON DUPLICATE KEY UPDATE` (MySQL) 及类似语法。

## 4. 验证计划 (TDD)
- **单元测试**: `test_sql_store_lifecycle` 验证连接初始化、写入、读取与删除。
- **集成测试**: 需本地 MySQL 运行环境验证。

## 5. 归档状态
- **状态**: `IN_PROGRESS`
- **执行人**: Master Orchestrator
- **开始日期**: 2026-04-29
