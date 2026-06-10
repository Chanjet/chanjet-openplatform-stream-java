# cowen-search Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **分离算力至侧车**：由于检索过程中需要跑轻量级的推理模型 (如 `cowen-search-embedding`)，将其留在主进程会导致主线程卡死和 OOM。故这部分功能下放给外部子进程实现。

## 时序流或关系图
*(暂无时序流图表)*
