# cowen-mcp-plugin

cowen-mcp-plugin 是对 Model Context Protocol (MCP) 协议的官方服务器端实现插件。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **标准化协议承载**：作为独立 Sidecar 进程运行，实现 MCP 协议的全集响应。
- **IPC 数据反向路由**：接收外部 Agent 发起的意图和能力调用并桥接至核心服务。

## ✅ 允许增加内容 (Allowed Additions)
- 扩充新的 MCP Tools/Resources 实现。
- 增加 MCP Client 端的身份认证机制。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止直接将 `cowen-services` 的具体实现静态链接到本插件中，必须通过 IPC 调用 Daemon 代理。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
