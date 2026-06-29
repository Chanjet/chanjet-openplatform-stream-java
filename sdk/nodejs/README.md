# Chanjet Stream Gateway Node.js SDK

畅捷通 Stream Gateway 官方 Node.js SDK。提供高性能的 Webhook-to-WebSocket 同步桥接客户端及业务分发机制。

## 安装

```bash
npm install @chanjet/connector-sdk
```

## 快速开始

```javascript
import { GatewayClient, MessageDispatcher } from '@chanjet/connector-sdk';

const client = new GatewayClient({
    appKey: '你的AppKey',
    appSecret: '你的AppSecret',
    // gatewayUrl: 'http://gateway-host:8080' // 可选，默认连接至畅捷通开放平台生产环境
});

const dispatcher = new MessageDispatcher();

// 注册应用票据处理器
dispatcher.onAppTicket((msg) => {
    console.log('AppTicket:', msg.appTicket);
    return true;
});

// 注册业务逻辑分发
client.onEvent(async (event) => {
    return await dispatcher.dispatch(event, '你的AppSecret');
});

client.start();
```

## 核心特性

- **智能连接管理**：自动处理 Nonce 获取、HMAC 签名握手。
- **自动重连**：内置指数退避（Exponential Backoff）与随机抖动（Jitter），自动处理 503 排队状态。
- **自动化解密**：`MessageDispatcher` 自动执行 AES-128-ECB 业务负载解密。
- **语义化路由**：支持基于 `boName` 和 `transactionType` 的精确消息分发。

## 许可证

MIT
