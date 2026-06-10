# cowen-store

cowen-store 提供统一的数据持久化和内存缓存管理服务。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **多态驱动器**：实现基于 SQLite、本地文件以及可选的 Redis 数据存取驱动层。
- **命名空间隔离**：保障不同租户/应用标识 (AppKey) 之间的数据隔离与读写安全。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的存储后端驱动实现 (如 MySQL, PostgreSQL 集群化存储)。
- 增加死信队列 (DLQ) 的文件级落地实现。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止存储未加密的纯文本敏感凭证信息（应先过加密逻辑）。
- **[FORBIDDEN]** 禁止在运行时直接修改数据库表结构（必须通过确定的 Migrations 控制）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
