# 概要设计文档 (High-Level Design - HLD)

> **项目名称**: cowen v0.3.0 - ISV Sidecar Evolution
> **阶段**: Phase 2 (HLD)
> **状态**: `DRAFT`

## 📖 目录 (Table of Contents)

1. [系统上下文视图 (含：分布式边车拓扑、数据流向全景)](./sections/01-context.md)
2. [部署与物理视图 (含：K8s Sidecar 部署、资源消耗预估)](./sections/02-deployment.md)
3. [架构决策记录 (ADR) (含：SQLx 选型对比、混合一致性策略)](./sections/03-adr.md)
4. [模块划分与依赖关系 (含：Store 抽象层、层级依赖图)](./sections/04-modules.md)
5. [非功能性设计 (含：TLS 强制、Prometheus 指标、分布式锁路径)](./sections/05-nfrs.md)

---
*本文档由 Master Orchestrator 自动生成，遵循 FDA (Document Fragmentation) 架构。*
