# cowen-monitor

cowen-monitor 负责收集、聚合及转储 Cowen 运行时的系统遥测与监控指标数据。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **运行态感知**：监控 CPU 使用率、内存泄漏预警及长连接存活状态。
- **审计与日志回溯**：集成并统一格式化输出系统级的错误溯源日志 (Audit Logs)。

## ✅ 允许增加内容 (Allowed Additions)
- 增加对 Prometheus 或 OpenTelemetry 标准的指标暴露接口。
- 增加日志滚动归档的清洗策略。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止在遥测上报中包含任何未脱敏的用户业务数据（Body/Query Param）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
