# cowen-signer

cowen-signer 是 Cowen 工具链提供的专属构建期 CLI 工具，用于对发布的二进制侧车插件及 WebAssembly 模块进行数字鉴权签名。

## 🛡️ 能力范围与边界 (Scope & Boundaries)
- **文件防篡改**：读取密钥并计算目标文件的防篡改校验和。
- **打包安全元数据**：自动生成 `plugin.json` 中的清单与数字签名。

## ✅ 允许增加内容 (Allowed Additions)
- 支持更多种类的公私钥格式（PEM, PK8 等）。
- 增强签名打包输出的灵活性。

## ❌ 禁止增加内容 (Forbidden Additions / Red Lines)
> **架构红线**：一旦突破以下边界，将可能导致 PR 审核被直接驳回，或引发严重的系统耦合。
- **[FORBIDDEN]** 禁止作为运行时依赖被 `cowen-daemon` 静态链接，它仅是个离线构建工具。

## 📚 内部文档索引 (Documentation Index)
针对该模块的开发细节、API 接口参考及核心架构设计，请参考 `docs/` 目录：
- [API Reference (`docs/api_reference.md`)](docs/api_reference.md) - 关键暴露接口与契约梳理。
- [Design Notes (`docs/design_notes.md`)](docs/design_notes.md) - 架构设计与历史决策说明。
