# cowen-store Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **SQLite 预写式日志 (WAL) 模式**：为保障多读一写并发下的吞吐量，启用了 WAL。
- **隔离层接口设计**：虽然当前强依赖 SQLite，但所有的操作都被封装在 `NativeStore` Trait 中，方便在 K8s 分布式模式下无缝切换为 Redis 驱动。

## 时序流或关系图
*(暂无时序流图表)*
