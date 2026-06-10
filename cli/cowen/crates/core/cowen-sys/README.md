# cowen-sys

cowen-sys 是 Cowen 对抗底层操作系统差异的核心系统抽象层 (System Abstraction Layer)。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **跨平台一致性屏蔽**：提供 Unix/Linux/macOS 与 Windows 底层 API 的统一封装。
- **强行接口对齐律**：绝对遵循所有涉及 OS Specific 的代码在全部目标平台均有同步签名。

## ✅ 允许增加内容 (Allowed Additions)
- 增加对新操作系统特性的适配调用（必须全平台声明齐平）。
- 改进基于系统底层的单例锁 (PID Lock) 算法。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止物理删除或截短其他操作系统的专属适配文件（如 `sys/windows.rs` 或 `sys/linux.rs`）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
