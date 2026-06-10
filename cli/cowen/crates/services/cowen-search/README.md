# cowen-search

cowen-search 是系统全局搜索策略的编排层，支撑面向云端语料库的智能化探索能力。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **联邦搜索分发**：根据当前的 Profile 策略，将搜索请求路由至本地缓存检索节点或云端服务。
- **上下文感知**：集成检索增强生成 (RAG) 的前置组装逻辑。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的多路召回 (Recall) 并发策略。
- 增加查询关键字纠错处理逻辑。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止将重量级的机器学习计算库 (如 `tch`、`onnxruntime`) 引入本模块（应该委托给 Sidecar 进程）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
