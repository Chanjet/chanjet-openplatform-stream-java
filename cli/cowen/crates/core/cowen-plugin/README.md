# cowen-plugin

cowen-plugin 是 Cowen 的统一侧车 (Sidecar) 进程与 Wasm 模块沙箱的管理引擎。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **子进程托管**：提供跨平台的子进程创建、僵尸进程回收管理能力。
- **IPC 桥接**：为外部进程提供本地 Socket 通信建立的控制流。

## ✅ 允许增加内容 (Allowed Additions)
- 改进与子进程握手保活的探活机制。
- 增加对不同打包格式 (ZIP, Tarball) 插件的热装载算法。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在 Crate 中直接硬编码特定 AI 模型的调用逻辑（此类逻辑应留在 Sidecar 二进制中）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
