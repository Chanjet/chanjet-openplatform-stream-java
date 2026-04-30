# 核心痛点与“一句话诉求” (Pain Points & Elevator Pitch)

## 核心痛点
- **无法分布式部署**：当前的配置存储在本地（Local File），这导致 Cowen 只能以单机模式运行，无法在云原生或多实例环境下实现配置共享和分布式部署。
- **能力受限**：现有的 Proxy 和 Webhook 能力相对薄弱，不足以支撑作为复杂 ISV 应用边车的要求。

## 一句话诉求 (Elevator Pitch)
将 cowen 升级为支持分布式存储（Redis/MySQL/PG/SQLServer）和混合存储模式的 **ISV 边车**，在增强 Proxy 与 Webhook 能力的同时，确保对历史功能的 100% 兼容。
