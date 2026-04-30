# 功能清单 (Feature List)

## 1. 存储层抽象 (解决分布式部署的物理隔离问题)
- **[Feature-01] 可插拔存储架构 (Pluggable Storage)**：定义统一的存储抽象接口，支持多种物理实现。
- **[Feature-02] 本地文件驱动 (Local Storage Driver)**：兼容 v0.2.x 的本地 `.yaml` 和 `.vault` 存储，作为默认后端。
- **[Feature-03] 共享存储驱动 (Shared Storage Driver)**：支持 MySQL, PostgreSQL, SQLServer, Redis 的物理连接与操作。
- **[Feature-04] 存储互斥锁机制 (Mutual Exclusion)**：确保系统启动时 Local 和 Shared 只能二选一，防止数据一致性冲突。

## 2. 混合存储能力 (解决短效数据的高性能与长效数据的可靠性矛盾)
- **[Feature-05] 多级数据映射 (Multi-level Mapping)**：支持配置哪些数据字段存放在 Redis（缓存层），哪些存放在 DB（持久化层）。
- **[Feature-06] 缓存穿透与同步 (Cache-Persistence Sync)**：Token 刷新后自动同步更新缓存与数据库。

## 3. Proxy & Webhook 增强 (解决边车作为生产入口的能力不足)
- **[Feature-07] ISV 商店应用适配**：支持商店应用的多租户（企业版）Token 刷新逻辑。
- **[Feature-08] 高性能 Webhook 转发**：支持异步队列缓冲与高可靠去壳转发。

## 4. 管理与迁移 (解决平滑升级与运维成本)
- **[Feature-10] 存储状态自检与管理 (Store Management)**：通过独立的 `store` 命令管理全局配置（Type, URL, Cache）并实时监控后端存储连接可用性。

---
*关联史诗：[BRD Epic-01, Epic-02](../../brd/sections/04-epics-boundaries.md)*
