# cowen-wasm-auth-storeapp

cowen-wasm-auth-storeapp 是为“商店应用模式 (Store App)”专供的鉴权逻辑 WebAssembly 插件。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **商店应用鉴权体系支持**：解析与处理商店应用特定的授权令牌 (Auth Tickets) 和验证回调逻辑。
- **沙箱级分发**：热更新 Wasm 避免重编译主程序。

## ✅ 允许增加内容 (Allowed Additions)
- 更新商店授权票据的验签规则。
- 调整 Ticket 解析格式容错。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止引入平台绑定相关的阻塞调用。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
