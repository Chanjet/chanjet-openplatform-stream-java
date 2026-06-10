# cowen-wasm-facade

cowen-wasm-facade 是 Cowen 架构中面向 WebAssembly (Wasm) 的门面适配层。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **沙箱隔离与调用注入**：作为 Wasmtime 的宿主环境 (Host) 封装层，将 Cowen 的宿主能力注入到 Wasm 实例中。
- **跨边界数据传递**：处理复杂的宿主语言 (Rust) 和 WebAssembly 线性内存之间的数据序列化/反序列化。

## ✅ 允许增加内容 (Allowed Additions)
- 向 Wasm 宿主注入新的安全受控的系统能力调用接口 (Host Functions)。
- 更新 Wasm 内存布局读写工具函数。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在宿主函数中直接提供具有破坏性的 OS 级调用（如无限制的 `std::process::Command`），所有功能必须审计防逃逸。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
