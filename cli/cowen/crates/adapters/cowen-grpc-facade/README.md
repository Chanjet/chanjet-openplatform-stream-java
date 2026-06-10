# cowen-grpc-facade

cowen-grpc-facade 是 Cowen 架构中的 gRPC 门面（Facade）适配层。它实现了底层 Core Capabilities 和 Services 的协议剥离。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **端点暴露**：定义和实现 `proto/` 中声明的 gRPC 微服务端点。
- **协议转换**：将外部发来的 protobuf 数据结构转换为系统内部的 Domain Models（实体结构）。

## ✅ 允许增加内容 (Allowed Additions)
- 增加基于 `.proto` 生成的端点服务实现。
- 增加 DTO (Data Transfer Object) 到 Domain Model 的互相转换实现。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 绝对禁止在此 Crate 内部实现任何存储或鉴权业务逻辑（必须委托给底层 Traits）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
