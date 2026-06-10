# cowen-sys Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **条件编译隔离**：利用 `#[cfg(target_os = ...)]` 对不同平台底层的 Socket API 和系统锁机制进行分发。强调“强行接口对齐律”，任何一端增加特有实现，另一端必须增加空实现，否则 CI 的 `make check-cross` 会直接失败。

## 时序流或关系图
*(暂无时序流图表)*
