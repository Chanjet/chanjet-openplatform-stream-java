# cowen-server

cowen-server 负责统筹 Cowen Daemon 中的各类网络及端点监听逻辑。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **IPC/gRPC 服务端**：启动本地 IPC 服务器，接收来自 `cowen-cli` 的指令并执行。
- **本地 Webhook/Proxy**：负责在本地启动用于转发和代理的 HTTP/WebSocket 接口。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的本地端口监听器（如监控 Metrics 端点）。
- 增加 HTTP/WebSocket 路由规则。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在 Server 中直接编写复杂的业务验证逻辑（应路由至 Services）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
