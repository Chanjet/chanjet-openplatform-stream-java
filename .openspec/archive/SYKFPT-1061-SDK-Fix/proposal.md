# 提案：修复 Java SDK 消息包装结构 (fix-sdk-message-wrapper)

## Why

**背景**：
- 在 Review 代码时发现，畅捷通开放平台推送的消息负载（`EventFrame.payload`）并非直接的密文，而是包含在 `{"encryptMsg": "..."}` 结构中的 JSON 字符串。
- 此外，消息头中可能不提供显式的加密算法标识，SDK 需要默认采用 AES 解密策略。

**当前状态**：
- `MessageDispatcher` 尝试直接将整个 `payload` 作为解密对象或原始 JSON 逻辑处理。
- 逻辑上依赖 `X-CJT-Encryption` 头，若缺失则跳过解密，导致处理失败。

**期望状态**：
- `MessageDispatcher` 能够解析 `{"encryptMsg": "..."}` 包装结构。
- 默认尝试 AES 解密 `encryptMsg` 字段中的内容。

## What Changes

- **修改 `MessageDispatcher.java`**:
    - 在分发前，先解析 `payload` 为 JSON 对象。
    - 检查是否存在 `encryptMsg` 字段。
    - 如果存在，则提取并使用 `CryptoUtils.aesDecrypt` 进行解密。
    - 解密后，再进行后续的业务类型提取和 POJO 转换。
- **更新测试用例**:
    - 更新 `MessageDispatcherTest`，确保模拟数据符合 `encryptMsg` 包装格式。

## Impact

### 受影响的代码
- `sdk/java/src/main/java/com/chanjet/connector/sdk/MessageDispatcher.java`

## 时间线评估
- 预计工作量：极小（< 1 小时）。
