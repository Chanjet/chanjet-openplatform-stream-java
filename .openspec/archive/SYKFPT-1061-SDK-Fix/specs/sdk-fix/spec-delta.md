# 规范差异：修复 SDK 消息包装结构

## MODIFIED Requirements

### Requirement: 消息透明解密
当 `GatewayClient` 配置了 `appSecret` 时，
SDK SHALL 能够根据算法规范（AES-128-CBC）对 `EventFrame.payload` 中 `encryptMsg` 字段包裹的内容执行解密。

#### Scenario: 包装结构解密成功
GIVEN 有效的 AES Key 和 IV
AND `payload` 格式为 `{"encryptMsg": "BASE64_ENCRYPTED_DATA"}`
WHEN 调用 `MessageDispatcher.dispatch`
THEN 应该自动解密并返回解密后的业务明文。

### Requirement: 签名校验
在处理推送消息前，SDK SHALL 能够验证消息签名的合法性。
对于带有 `encryptMsg` 包装的消息，
签名校验 SHALL 针对原始的包装字符串（即 `encryptMsg` 字段所属的 JSON）进行。
