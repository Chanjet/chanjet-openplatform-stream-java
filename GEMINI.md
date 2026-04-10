# 畅捷通 Stream Gateway 工程规范 (Engineering Standards)

## 🛠️ 开发模式 (Development Mode)
- **TDD Mandatory**: 所有代码实现 **必须** 采用测试驱动开发 (TDD) 方式。严禁在没有对应失败测试用例的情况下编写生产代码。
- **Test Integrity**: 代码开发完成后，任何对现有测试用例 (Test Cases) 的修改行为 **必须进行复盘**。严禁通过修改测试来掩盖生产代码的错误或不兼容变更。

## 🏗️ 架构约束 (Architectural Constraints)
- **SPI First**: 核心逻辑仅依赖 `connector-api` 定义的接口。
- **Polyglot Ready**: 协议定义必须位于 `proto/` 目录下，且由 Protobuf 驱动。
- **Open-Closed Principle**: 未来开发必须严格遵守开闭原则（对扩展开放，对修改关闭），务必保证原有功能不受损。

---
**提示**：本规范具有最高优先级，所有 AI 代理及人类开发者应严格遵守。
