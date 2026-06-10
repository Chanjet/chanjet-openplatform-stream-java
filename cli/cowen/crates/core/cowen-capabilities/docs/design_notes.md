# cowen-capabilities Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **系统的灵魂中枢**：本项目一切架构的基础。按照依赖倒置原则 (IoC)，任何层级只能依赖 `capabilities` 中的 Trait。这不仅彻底消除了 Rust 中由于 Crate 互相引用导致的编译错误，而且极大降低了 TDD 中 Mock 对象生成的成本。

## 时序流或关系图
*(暂无时序流图表)*
