# Module: connector-core

## 1. 模块领域
本模块是网关的 **大脑 (Kernel)**，包含所有的 **业务规则 (Business Rules)**、**状态机** 和 **分发编排** 逻辑。

## 2. 能力范围
- 消息分发策略：本地优先、P2P 转发重试。
- 弹性管理：双层并发限流（Token Bucket）、熔断保护。
- 状态机：连接生命周期监控、自愈逻辑（Tolerance Manager）。
- 负载均衡算法：Random Load Balancer。

## 3. 准入规范
- **适合加入**: 消息流转的逻辑、核心算法、状态切换逻辑、业务校验规则。
- **严禁加入**: 任何与具体协议（如 WebSocket Session、HTTP Controller）或存储（Redis）直接挂钩的代码。本模块应只面向 `connector-api` 编程。
