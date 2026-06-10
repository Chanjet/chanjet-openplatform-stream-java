# cowen-grpc-facade Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **严格的 Facade 模式**：通过 `tonic` 实现 proto 中定义的接口，但内部只执行简单的 DTO 转换，并立刻将调用委托给 `cowen-capabilities` 的抽象特征 (Traits)。这种防腐层设计保护了我们的内核心服务不被外部生成的代码污染。

## 时序流或关系图
*(暂无时序流图表)*
