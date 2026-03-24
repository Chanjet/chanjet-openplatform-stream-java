# Node.js SDK 规范 (Node.js SDK Specification)

> **修订日期**: 2026-03-20
> **版本**: v0.1.0

---

## 1. 概述 (Overview)
Node.js SDK 是为 Node.js 技术栈的 ISV 提供的官方集成包，旨在实现与 Java SDK 等同的弹性连接与业务分发能力。

---

## 2. 核心需求 (Core Requirements)

### Requirement: Node.js SDK 核心客户端
SDK SHALL 提供一个 `GatewayClient` 类，负责与 Stream Gateway 建立并维护持久连接。

#### Scenario: 自动获取 Nonce 并签名连接
GIVEN 配置了有效的 `appKey` 和 `appSecret`
WHEN 调用 `client.start()`
THEN SDK SHALL 先通过 HTTP 获取 Nonce
AND 使用 HMAC-SHA256 算法生成签名
AND 通过 WebSocket 建立连接。

#### Scenario: 智能重连策略
GIVEN WebSocket 连接异常断开
WHEN `running` 状态为 true
THEN SDK SHALL 根据错误类型执行不同的重连策略：
- 401/403: 停止重连并抛出错误。
- 503/429: 进入排队模式（固定 5-15s 随机延迟）。
- 其他: 按照指数退避（Max 60s）进行重连。

### Requirement: 业务消息分发
SDK SHALL 提供 `MessageDispatcher` 机制，自动处理业务数据的加解密与分发。

#### Scenario: 自动解密业务负载
GIVEN 收到包含 `encryptMsg` 字段的事件帧
WHEN 分发消息时
THEN SDK SHALL 使用 `appSecret` 前 16 位进行 AES-128-ECB 解密
AND 将解密后的 JSON 转换为对应的消息对象。

#### Scenario: 语义化路由 (APP_NOTICE)
GIVEN 收到类型为 `APP_NOTICE` 的消息
WHEN 业务内容包含 `boName: "GoodsIssue"`
THEN SDK SHALL 优先查找注册在 `"APP_NOTICE:GoodsIssue"` 下的处理器进行处理。
