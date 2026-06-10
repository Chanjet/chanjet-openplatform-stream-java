# cowen-daemon

cowen-daemon 是整个 Cowen 系统的核心守护进程，负责承载和调度所有的后台微服务及侧车组件。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **常驻后台运行**：与 CLI 进程分离，常驻内存，管理长连接（例如 WebSocket 流代理）。
- **插件侧车治理**：拉起并维护独立的 Sidecar 进程（如 AI 搜索、MCP 插件）或 Wasm 沙箱，处理它们的生命周期。
- **服务组装与编排**：作为控制平面的中枢，组装 `services` 层提供的各项功能。

## ✅ 允许增加内容 (Allowed Additions)
- 增加后台任务调度器。
- 增加对新 Sidecar 插件生命周期的托管逻辑。
- 组装新的底层 Service。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止将 Terminal UI 输出逻辑写入 Daemon（Daemon 应只记录日志）。
- **[FORBIDDEN]** 禁止越过 `cowen-capabilities` 强依赖具体的服务实现结构体。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
