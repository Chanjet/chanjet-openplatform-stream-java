# 提案：SDK 增强 - 支持消息解密与业务分发 (SYKFPT-1061)

## Why

**背景**：
- 畅捷通开放平台的消息推送（如 T+ 生产订单消息）通常是加密且带签名的。
- 当前 SDK (`GatewayClient`) 仅提供原始 `EventFrame` 的推送，ISV 需要自行实现复杂的解密、验签和消息分发逻辑。
- 提高 SDK 的易用性，减少 ISV 的重复开发成本，是提升平台生态竞争力的关键。

**当前状态**：
- SDK 仅支持通过 `onEvent(EventHandler)` 接收原始消息。
- 缺乏 AES 解密和 SHA256 验签的工具类。
- 缺乏基于消息类型（`msgType`）的自动分发机制。

**期望状态**：
- SDK 内置 `MessageDispatcher`，支持根据消息类型自动分发到对应的 POJO 处理器。
- 提供透明的解密与验签支持，ISV 只需关注业务逻辑。
- 支持常见的业务消息模型（如 `manufactureOrderMsg`）的结构化定义。

## What Changes

- **新增 `CryptoUtils`**: 实现 AES-128-CBC 解密和 SHA256 签名验证。
- **新增 `MessageDispatcher`**: 提供消息类型注册、POJO 自动转换及分发能力。
- **新增 `BaseMessage` 抽象**: 定义统一的业务消息基类。
- **扩展 `GatewayClient`**: 支持接入 `MessageDispatcher`，并在接收到消息时自动执行解密、验签和分发流程。

## Impact

### 受影响的规范
- `connector-api` - 无需变更。
- `connector-common` - 无需变更。
- `sdk/java` - 主要变更点。

### 受影响的代码
- `sdk/java/src/main/java/com/chanjet/connector/sdk/GatewayClient.java` - 扩展对分发器的支持。
- `sdk/java/src/main/java/com/chanjet/connector/sdk/EventHandler.java` - 保持兼容，但推荐使用新的分发模式。

### 用户影响
- ISV 接入成本大幅降低。
- 现有 `onEvent` 接口保持兼容。

### API 变更
- 无破坏性变更。
- 新增 `GatewayClient.useDispatcher(MessageDispatcher)` 接口。

## 时间线评估
- 预计工作量：中（3-4 天）。

## 风险
- **加解密兼容性风险**：不同产品的 AES Key/IV 生成规则可能略有差异。
  - *缓解方案*：提供可配置的 `CryptoConfig`。
- **性能风险**：大流量下的解密与反射开销。
  - *缓解方案*：使用高性能的 Jackson 对象映射，并在必要时优化分发查找逻辑。
