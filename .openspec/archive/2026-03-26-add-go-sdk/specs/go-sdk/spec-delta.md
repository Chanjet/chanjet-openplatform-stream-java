# Specification Delta: 增加 Go SDK (add-go-sdk)

## ADDED Requirements

### Requirement: Go 语言 SDK 支持
项目 SHALL 提供 Go 语言版本的客户端 SDK，位于 `sdk/go` 目录下。

#### Scenario: 核心功能对等性
GIVEN 使用 Go SDK 接入
WHEN 与网关建立连接并接收消息
THEN Go SDK SHALL 表现出与 Java/Node.js SDK 完全一致的行为：
- 自动处理 Nonce 获取与 HMAC 签名。
- 自动生成符合 `{appKey}@{hostname}_{pid}_{random}` 格式的 ClientID。
- 遵循相同的指数退避重连算法。
- 默认支持 AES-128-ECB 解密独立密钥（encryptKey）。

### Requirement: Go SDK 消息分发
Go SDK SHALL 提供并发安全的 `MessageDispatcher` 机制。

#### Scenario: 并发处理
GIVEN 注册了多个消息处理器
WHEN 高频接收到不同类型的事件帧
THEN Go SDK SHALL 支持在 Goroutine 中并发调用处理器，确保高性能。

#### Scenario: 嵌套解析支持
GIVEN 收到包含 `bizContent` 的解密报文
THEN Go SDK SHALL 能够将 JSON 正确映射到对应的 Struct 字段（如 `appTicket`）。
