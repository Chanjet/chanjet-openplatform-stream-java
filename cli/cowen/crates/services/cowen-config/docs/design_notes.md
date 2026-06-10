# cowen-config Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **配置优先级融合机制**：CLI Flags > 环境变量 > 本地 `config.yaml`。确保了开发者在调试时可以轻易地覆写参数。
- **配置变更事件订阅**：未来计划引入基于 Channel 的发布-订阅模式，当配置文件更新时，热加载生效而无需重启 Daemon。

## 时序流或关系图
*(暂无时序流图表)*
