# cowen-config

cowen-config 是系统的配置管理服务，负责聚合来自不同源的配置树并向全系统派发配置项。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **文件系统落盘**：读取与解析本地配置文件（如 `~/.cowen/config.yaml`）。
- **合并策略**：整合环境变量、CLI 运行时传递参数及本地存储配置项的覆盖规则。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的配置项反序列化模型。
- 增加从远程配置中心拉取配置的支持。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止对外暴露可能包含明文敏感 Key 的全量子配置对象，必须对敏感字段提供脱敏访问方法。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
