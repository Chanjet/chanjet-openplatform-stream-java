# cowen-wasm-auth-storeapp Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **跨越 Wasm 边界的 JSON 交互**：考虑到 FFI 的字符串编解码复杂性，我们采用序列化的 JSON 字节数组作为宿主与插件交互的主体载体，简化鉴权凭据的映射逻辑。

## 时序流或关系图
*(暂无时序流图表)*
