# 高阶核心史诗与业务边界 (High-Level Epics & Business Boundaries)

## 核心史诗 (Epics)
- **[Epic-01] 抽象化存储引擎 (Pluggable Storage Engine)**：支持 Local File, Redis, MySQL, PostgreSQL, SQLServer 的抽象接口实现。
- **[Epic-02] 混合动力存储模式 (Hybrid Storage Mode)**：实现“缓存（Redis/Memory）+ 持久化（DB）”的协同工作模式，确保短效数据高性能、长效数据高可靠。
- **[Epic-03] ISV 全模式支持 (Universal App Support)**：深度适配“自建应用”与“商店应用”的身份认证与 Token 刷新逻辑。
- **[Epic-04] 生产级 Proxy & Webhook 增强**：提升转发效率，支持更复杂的路由策略与重试机制。
- **[Epic-05] 历史资产继承 (Legacy Compatibility)**：平滑迁移旧版配置文件，确保老用户无感升级。

## 业务边界 (Boundaries)
- **反向边界 (What NOT to do)**:
  - 不提供数据库自身的运维管理功能（如备份、扩容）。
  - 不负责 ISV 应用的业务逻辑实现。
  - **全局一致性存储**: 存储方式与应用部署实例（Application Instance）全局绑定，而非按 Profile 粒度配置。不支持在同一应用中，一部分 Profile 使用本地存储，另一部分使用数据库存储。要么全部在硬盘上配置，要么全部在远程存储上配置。
- **容错底线**:
  - 存储后端不可用时，Sidecar 必须提供降级通知，而非无响应挂死。
  - 任何存储切换或配置动作必须通过独立的 `store` 命令显式触发，禁止在 Profile 初始化过程中隐式修改全局存储配置。

---
*关联原始访谈：[关键干系人与约束](../initial_client_interview/sections/04-stakeholders-constraints.md)*
