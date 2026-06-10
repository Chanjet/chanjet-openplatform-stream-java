# cowen-wasm-auth-selfbuilt

cowen-wasm-auth-selfbuilt 是为“自建应用模式 (Self-built App)”专供的鉴权逻辑 WebAssembly 插件。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **自建应用签名运算**：封装处理自建应用的公私钥证书验证及身份签名加密流程。
- **沙箱级分发**：以 `.wasm` 格式进行二进制分发与独立热更新。

## ✅ 允许增加内容 (Allowed Additions)
- 更新针对自建应用的认证哈希算法实现。
- 补充证书有效期自校验逻辑。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止引入任何 `std::thread` 或 `tokio` 异步运行时（Wasm32 不支持）。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
