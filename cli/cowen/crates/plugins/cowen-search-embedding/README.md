# cowen-search-embedding

cowen-search-embedding 是 Cowen 内置的独立 Rust-native 本地语义向量化 (Embedding) 与 AI 搜索侧车 (Sidecar) 插件。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **本地向量化计算**：在本地或内存模型中进行 Embedding 计算提取。
- **安全与防篡改**：独立执行以防内存污染或系统崩溃影响核心 Daemon。

## ✅ 允许增加内容 (Allowed Additions)
- 引入更高效的 ONNX 或 Candle 算子后端。
- 增加对新 embedding 模型的量化支持 (如 int8/fp16)。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在此侧车内执行任何修改本地核心数据库 (`sqlite`) 的越权行为。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
