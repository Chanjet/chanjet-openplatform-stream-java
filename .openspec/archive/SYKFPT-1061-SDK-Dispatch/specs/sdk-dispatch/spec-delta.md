# 规范差异：SDK 消息解密与业务分发

## ADDED Requirements

### Requirement: 消息透明解密
当 `GatewayClient` 配置了 `appSecret` 且 `EventFrame.payload` 被加密时，
SDK SHALL 能够根据算法规范（AES-128-CBC）执行解密。

#### Scenario: 解密成功
GIVEN 有效的 AES Key (AppSecret 前 16 位) 和 IV (AppSecret 后 16 位)
WHEN 调用 `CryptoUtils.aesDecrypt`
THEN 应该返回 UTF-8 编码的消息原文。

### Requirement: 消息自动分发 (Message Dispatching)
SDK SHALL 提供 `MessageDispatcher`，允许 ISV 按消息类型注册 POJO 类型及处理器。

#### Scenario: 注册并分发 T+ 生产订单消息
GIVEN 注册了 `manufactureOrderMsg` 到 `ManufactureOrderMessage.class`
AND 注册了一个对应的 `MessageHandler`
WHEN 接收到 `msgType` 为 `manufactureOrderMsg` 的推送
THEN SDK 应该自动解析 `payload` 为 `ManufactureOrderMessage` 对象
AND 调用对应的处理器处理。

### Requirement: 签名校验
在处理推送消息前，SDK SHALL 能够验证消息签名的合法性，以防止消息篡改。

#### Scenario: 签名验证通过
GIVEN 正确的消息体、nonce、timestamp 及 AppSecret
WHEN 执行签名计算
THEN 计算出的摘要值应与消息头中的 `X-CJT-Signature` 一致。

## MODIFIED Requirements

### Requirement: 增强的事件处理流程
WHEN `GatewayClient` 收到 `event` 类型的帧,
IF 已配置 `MessageDispatcher`
THEN SDK SHALL 优先尝试通过 `MessageDispatcher` 执行分发逻辑。
ELSE SHALL 继续使用 `onEvent` 配置的原始处理器。
