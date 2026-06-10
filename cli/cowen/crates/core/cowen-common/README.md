# cowen-common

cowen-common 提供了 Cowen CLI 和 Daemon 所需的跨模块基础常量和通用工具函数。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **全局常量与定义**：维护全系统共享的错误类型 (Error Types)、日志宏级别控制。
- **通用算法支持**：包含时间戳转换、字符串清洗等无副作用的基础逻辑包。

## ✅ 允许增加内容 (Allowed Additions)
- 增加通用的算法库封装（如 MD5/SHA 工具，Base64 工具）。
- 扩充全局共用 Enum 异常类型。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止引入任何业务逻辑专用的模型（如订单模型、用户模型）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
