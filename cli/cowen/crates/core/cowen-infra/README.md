# cowen-infra

cowen-infra 是 Cowen 的基础技术架构支撑层。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **底层传输与异步基建**：基于 reqwest 封装带有限流及断路器的弹性 HTTP 客户端。
- **通道管道编排**：提供 MPSC 或广播 (Broadcast) 异步队列的高级管理机制。

## ✅ 允许增加内容 (Allowed Additions)
- 升级异步运行时配置或网络连接池算法。
- 封装底层 WebSocket 断线重连框架。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止出现特定业务 API 的路由地址硬编码（应由上层通过配置传入）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
