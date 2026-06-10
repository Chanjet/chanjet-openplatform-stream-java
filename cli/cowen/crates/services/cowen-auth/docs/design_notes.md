# cowen-auth Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **多鉴权提供商 (AuthProvider) SPI 机制**：由于我们有 Self-built 和 Store-App 两种截然不同的生态认证方式，所以将所有的 Token 解析抽象成了统一的 AuthProvider。
- **Token 主动保活**：引入了心跳刷新线程，确保 Access Token 在过期前主动去中心端续期，避免请求阻断。

## 时序流或关系图
*(暂无时序流图表)*
