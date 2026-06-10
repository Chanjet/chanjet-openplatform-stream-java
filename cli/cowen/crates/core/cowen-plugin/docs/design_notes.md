# cowen-plugin Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **管道与标准流管理**：为了防止 Sidecar 进程成为孤儿进程 (Orphan Process)，当主进程退出时，管道 (`stdin/stdout`) 会立即断开，促使插件自我销毁。

## 时序流或关系图
*(暂无时序流图表)*
