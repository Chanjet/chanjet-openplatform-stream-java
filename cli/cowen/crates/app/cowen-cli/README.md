# cowen-cli

cowen-cli 是 Stream Gateway 工具链的前端入口，采用 "Thin CLI"（瘦客户端）架构设计。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **用户交互入口**：解析终端命令行参数（基于 `clap`），提供统一的控制台交互体验。
- **IPC 代理层**：CLI 进程不直接运行耗时或有状态的逻辑，而是将绝大部分命令通过 IPC 机制转发至后台运行的守护进程 (`cowen-daemon`) 执行。
- **环境隔离**：负责在请求发起前加载当前 Profile（default/inte/prod）。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的 CLI 命令和参数解析。
- 增加新的 IPC 客户端调用封装。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在 CLI 中硬编码业务鉴权逻辑（必须通过 IPC 委托给 Daemon）。
- **[FORBIDDEN]** 禁止直接连接数据库或 Redis。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
