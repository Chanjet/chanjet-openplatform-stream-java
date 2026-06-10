# cowen-auth

cowen-auth 是 Cowen 的认证鉴权中枢微服务。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **多模式认证支持**：统一处理不同环境模式下的认证逻辑，包括自建应用、OAuth2 应用以及商店应用体系。
- **Token 生命周期治理**：负责与云端交互，实现 Access Token 的颁发、续期机制与吊销。

## ✅ 允许增加内容 (Allowed Additions)
- 增加新的第三方 AuthProvider 策略实现。
- 增加 Token 轮换、过期主动探测逻辑。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止直接硬编码 SQL 进行存储（必须调用 `cowen-capabilities::NativeStore` 契约）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
