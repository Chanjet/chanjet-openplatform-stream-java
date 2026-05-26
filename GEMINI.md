# 畅捷通 Stream Gateway 工程规范 (Engineering Standards)

## 🛠️ 开发模式 (Development Mode)
- **TDD Mandatory**: 所有代码实现 **必须** 采用测试驱动开发 (TDD) 方式。严禁在没有对应失败测试用例的情况下编写生产代码。
- **Test Integrity**: 代码开发完成后，任何对现有测试用例 (Test Cases) 的修改行为 **必须进行复盘**。严禁通过修改测试来掩盖生产代码的错误或不兼容变更。

## 🏗️ 架构约束 (Architectural Constraints)
- **SPI First**: 核心逻辑仅依赖 `connector-api` 定义的接口。
- **Polyglot Ready**: 协议定义必须位于 `proto/` 目录下，且由 Protobuf 驱动。
- **Open-Closed Principle**: 未来开发必须严格遵守开闭原则（对扩展开放，对修改关闭），务必保证原有功能不受损。

## 📝 文档规范 (Documentation Standards)
- **Doc Sanitization Mandatory**: 文档（Markdown, PRD, API 规范等）中涉及 Token、Secret、密钥等敏感数据的示例值 **必须** 使用 `<VALUE_NAME>` 占位符展示（如 `"appTicket": "<APP_TICKET>"`）。严禁在文档中出现真实或随机生成的敏感字面量。
- **Readability First**: 非敏感的业务数据（如 `msg_id`, `timestamp`, `id`, `name`）建议使用易读的字面量示例，以增强文档的参考价值。

## 💻 跨平台安全开发规范 (Cross-Platform Safety Development)
- **强行接口对齐律 (Interface Alignment)**：任何人在扩展底层平台专属能力时，**必须**在所有支持的目标平台（macOS、Linux、Windows）同步重载声明，并使用 `unimplemented!()` 占位或提供等价实现，绝对保障全平台 Crate 在任何时刻均可成功编译。
- **禁止擅自物理清理 (Anti-Deletion)**：AI 代理与人类开发者在单端环境下重构或优化代码时，**绝对禁止**物理删除或截短其他操作系统的专属适配文件（如 `sys/windows.rs` 或 `sys/linux.rs`），确保多端资产完整性。
- **强制本地静态回归 (Static Regression Check)**：凡涉及 `sys` 系统抽象层及其实现目录的修改，在提交代码或标记任务完成前，**必须**在控制台执行 `make check-cross`（或在具备相应交叉工具链的环境下）验证多目标端的静态语法及类型校验。

---
**提示**：本规范具有最高优先级，所有 AI 代理及人类开发者应严格遵守。
