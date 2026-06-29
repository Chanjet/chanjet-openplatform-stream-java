# Chanjet Stream Gateway Rust SDK

畅捷通 Stream Gateway 官方 Rust SDK。提供高性能的 Webhook-to-WebSocket 同步桥接客户端及业务分发机制。基于 `tokio` 异步运行时构建，提供极高的并发性能。

## 安装

在您的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
connector-sdk = { git = "https://github.com/Chanjet/chanjet-openplatform-stream-rust.git", branch = "master" }
```

## 快速开始

```rust
use connector_sdk::{GatewayClient, ClientOptions};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 准备配置项
    let options = ClientOptions {
        app_key: env::var("APP_KEY").expect("APP_KEY missing"),
        app_secret: env::var("APP_SECRET").expect("APP_SECRET missing"),
        encrypt_key: env::var("ENCRYPT_KEY").ok(),
        gateway_url: "wss://stream-open.chanapp.chanjet.com".to_string(), // 可选
    };

    // 2. 初始化客户端
    let client = GatewayClient::new(options);
    
    // 3. 注册处理器
    {
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();
        
        // 注册应用票据处理器
        dispatcher.on_app_ticket(|msg| {
            println!("收到应用票据: {}", msg.biz_content.app_ticket);
            true
        });

        // 注册业务逻辑分发 (例如订单状态通知)
        dispatcher.on_order_status(|msg| {
            println!("收到订单支付成功消息: {}", msg.biz_content.order_no);
            // 务必返回 true，SDK 会自动向网关发送 ACK
            true
        });
    }

    // 4. 启动客户端
    client.start().await?;

    // 阻塞主线程等待退出信号...
    tokio::signal::ctrl_c().await?;
    client.stop();

    Ok(())
}
```

## 核心特性

- **智能连接管理**：自动处理 Nonce 获取、HMAC 签名握手。
- **自动重连**：内置指数退避（Exponential Backoff）与随机抖动（Jitter），自动处理网络波动。
- **自动化解密**：`MessageDispatcher` 自动执行 AES-128-ECB 业务负载解密，且内置 `SanitizeKey` 净化不可见字符。
- **语义化路由**：支持基于 `boName` 和 `transactionTypeEnum` 的精确消息分发。
- **消息可靠性机制**：通过业务闭包的返回值（`true`/`false`）控制是否下发 `sys_ack`，保障至少一次投递。也支持自定义 `DlqProvider` 防止漏单。

## 开发指南与示例

### 1. 接收推送与自动解密

SDK 中的 `dispatcher` 会帮您自动完成数据解密。通过 `on_app_notice` 和 `on_order_status` 等方法，您可以快速监听您关心的业务对象事件。闭包要求返回一个布尔值以决定是否下发 ACK。

### 2. ACK、断线重连与幂等处理

- **ACK (确认机制)**：在处理器中返回 `true`，SDK 会自动构造并发送 `sys_ack` 帧给网关。若返回 `false`，则不回复成功，网关将会稍后重推。
- **断线重连**：内置心跳保活与异常熔断机制。网络断开会自动触发带有指数退避参数的重新连接。
- **幂等处理**：由于“至少投递一次”策略，存在重复推送的可能性。请务必使用事件的唯一 ID 进行去重。

## 许可证

MIT

## 更新日志 (Changelog)

### v0.1.0 (2026-06)
- **首发**: 提供基于 `tokio` 和 `tokio-tungstenite` 的高性能 Webhook-to-WebSocket 桥接客户端。
- **机制**: 内置死信队列 (DLQ)、指数退避与随机抖动重连、自动加解密及消息签名防篡改。
- **路由**: 提供 `MessageDispatcher`，支持针对事件的闭包拦截与业务分发（如 `on_app_ticket`、`on_order_status`），并内置秘钥净化机制（SanitizeKey）。
