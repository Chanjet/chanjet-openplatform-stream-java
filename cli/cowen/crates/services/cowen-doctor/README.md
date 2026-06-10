# cowen-doctor

cowen-doctor 提供了面向系统健康、运行环境及网络连通性的探针与诊断服务。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **环境检查**：验证本地操作系统支持、缺失依赖以及文件系统权限。
- **网络诊断**：探测对核心云端服务（如 OpenAPI 与 Stream Gateway）的连通性。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的故障诊断探针（如磁盘 I/O 测速、DNS 污染检测）。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在诊断过程中自动尝试进行具有破坏性的数据修复操作。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
