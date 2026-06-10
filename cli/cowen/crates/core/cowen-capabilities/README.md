# cowen-capabilities

cowen-capabilities 定义了整个系统中模块相互通信的顶层契约 (Traits) 和接口协定。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **依赖倒置与解耦**：将所有高层服务抽象为 Traits (例如 `NativeAuth`, `NativeConfig`, `NativeStore`)。
- **系统核心抽象**：统一系统中所有的边界能力规范。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的全系统级别特征协议 (Trait) 声明。
- 增加针对特征抽象的通用 Mock 实现用于测试。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 绝对禁止在此 Crate 引入 `services` 的底层具体实现层逻辑，只能包含抽象定义。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
