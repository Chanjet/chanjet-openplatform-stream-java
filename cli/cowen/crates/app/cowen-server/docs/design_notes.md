# cowen-server Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **端口多路复用**：需要同时处理本地 IPC Socket (Unix Domain Socket / Named Pipes) 以及可能的 HTTP Proxy 请求。
- **协议无状态化**：它仅仅作为请求的转发和协议解析边界，任何复杂的业务流转应丢给后端的 Service 实例。

## 时序流或关系图
*(暂无时序流图表)*
