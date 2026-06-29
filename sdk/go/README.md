# Chanjet Stream Gateway Go SDK

畅捷通 Stream Gateway 官方 Go SDK。提供高性能的 Webhook-to-WebSocket 同步桥接客户端及业务分发机制。

## 安装

```bash
go get github.com/Chanjet/chanjet-openplatform-stream-go@v0.2.0
```
> 注：当前模块名为 `com.chanjet/connector-sdk-go`。

## 快速开始

```go
package main

import (
	"log"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"com.chanjet/connector-sdk-go/pkg/sdk"
)

func main() {
	// 1. 初始化客户端
	client := sdk.NewGatewayClient(sdk.ClientOptions{
		AppKey:     "<APP_KEY>",
		AppSecret:  "<APP_SECRET>",
		// GatewayURL: "wss://stream-open.chanapp.chanjet.com", // 可选，默认连接至畅捷通开放平台生产环境
	})

	// 2. 初始化分发器
	dispatcher := sdk.NewMessageDispatcher()

	// 3. 注册应用票据处理器
	dispatcher.OnAppTicket(func(msg protocol.AppTicketMessage) bool {
		log.Printf("收到 AppTicket: %s", msg.AppTicket)
		return true
	})

	// 4. 注册业务逻辑分发 (例如销货单)
	dispatcher.OnAppNotice("GoodsIssue", "", func(msg protocol.AppNoticeMessage) bool {
		log.Printf("销货单变更数据: %+v", msg.BizContent)
		// 务必返回 true，SDK 会自动向网关发送 ACK
		return true
	})

	// 5. 绑定分发器并启动客户端
	client.UseDispatcher(dispatcher)
	client.Start()

	// 阻塞主协程
	select {}
}
```

## 核心特性

- **智能连接管理**：自动处理 Nonce 获取、HMAC 签名握手。
- **自动重连**：内置指数退避（Exponential Backoff）与随机抖动（Jitter），自动处理 503 排队状态，支持自定义 `ReconnectInterval` 和 `MaxBackoff`。
- **自动化解密**：`MessageDispatcher` 自动执行 AES-128-ECB 业务负载解密。
- **语义化路由**：支持基于 `boName` 和 `transactionTypeEnum` 的精确消息分发。
- **消息可靠性机制 (DLQ)**：支持设置 `DlqProvider`，在网络或业务异常时暂存消息，防止漏单。

## 开发指南与示例

### 1. 接收推送与自动解密

SDK 中的 `MessageDispatcher` 会帮您自动完成数据解密（AES-128-ECB）并根据消息类型进行路由。通过 `OnAppNotice` 和 `OnOrderStatus` 等方法，您可以快速监听您关心的业务对象事件。

### 2. ACK、断线重连与幂等处理

- **ACK (确认机制)**：在处理器中返回 `true`，SDK 会自动构造并发送 `sys_ack` 帧给网关，确认消息已消费。若返回 `false` 或引发 Panic（需自行 recover），则会回复失败状态（HTTP 500）。
- **断线重连**：SDK 内置了心跳保活机制。网络波动导致的断开会自动重新连接，无需人工干预。
- **幂等处理**：由于消息保证“至少投递一次”（At-Least-Once），可能存在重复推送。请务必使用事件的 `MsgID` 进行去重。

### 3. 使用死信队列 (DLQ) 防漏单

在 `ClientOptions` 中配置 `DlqProvider` 接口的实现类。当 SDK 接收到事件后，会首先调用 `Store` 暂存到本地（如 Redis 或 MySQL）。待业务处理完毕后，再调用 `Remove` 移除。如果暂存失败，SDK 会拒绝本次分发，促使云端稍后重试。

## 许可证

MIT

## 更新日志 (Changelog)

### v0.2.0 (2026-06)
- **新增**: 死信队列 (DLQ) 机制，支持通过配置 `DlqProvider` 接口在分发异常时暂存消息，防止意外漏单。
- **新增**: 断线重连加入指数退避 (Exponential Backoff) 和随机抖动机制，可通过 `ClientOptions` 自定义 `MaxBackoff` 和 `ReconnectInterval`。
- **新增**: 增强 `MessageDispatcher` 路由能力，加入 `SetFallbackHandler` 兜底处理器，以及针对常见业务的快捷注册方法 `OnOrderStatus`、`OnAppNotice`。
- **优化**: 提升解密引擎鲁棒性，加入 `SanitizeKey` 机制清理配置中可能掺杂的 Zero-Width Space 控制符，并向上对齐支持 32位 Hex 格式秘钥作为等价 AES-128 秘钥。
