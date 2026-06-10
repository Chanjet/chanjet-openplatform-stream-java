# cowen-mcp-plugin Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **独立生命周期**：作为 MCP 服务端，它往往被外部的主流 LLM IDE (Cursor、Windsurf) 唤起，它的生命周期可以由 IDE 接管，也可以由 Daemon 拉起，因此该组件需要极端的自适应性。

## 时序流或关系图
*(暂无时序流图表)*
