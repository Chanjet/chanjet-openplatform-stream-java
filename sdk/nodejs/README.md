# Chanjet Stream Gateway Node.js SDK

畅捷通 Stream Gateway 官方 Node.js SDK。提供高性能的 Webhook-to-WebSocket 同步桥接客户端及业务分发机制。

## 安装

```bash
npm install @chanjet_openplatform/open_stream_sdk
```

## 快速开始

```javascript
import { GatewayClient, MessageDispatcher } from '@chanjet_openplatform/open_stream_sdk';

const client = new GatewayClient({
    appKey: '你的AppKey',
    appSecret: '你的AppSecret',
    // gatewayUrl: 'https://stream-open.chanapp.chanjet.com' // 可选，默认连接至畅捷通开放平台生产环境
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
## 开发指南与示例 (Node.js)

### 1. 接收推送与自动解密

SDK 中的 `MessageDispatcher` 会帮您自动完成数据解密（AES-128-ECB）并根据消息类型进行路由：

```javascript
// 注册销货单 (GoodsIssue) 通知处理
dispatcher.onAppNotice('GoodsIssue', async (msg) => {
    // msg.bizContent 已经是解密后并解析为 JSON 的对象
    console.log('销货单变更数据:', msg.bizContent);
    
    // 务必返回 true，SDK 会自动向网关发送 ACK
    return true; 
});
```

### 2. 根据推送拉取详情

开放平台推送的消息通常只包含核心状态和 ID。在收到推送后，您可以调用开放平台 API 拉取完整详情：

```javascript
dispatcher.onAppNotice('GoodsIssue', async (msg) => {
    const { id, action } = msg.bizContent;
    
    if (action === 'create') {
        // 伪代码：使用您的业务 Token 和单据 ID 调用畅捷通 OpenAPI 获取详情
        // const detail = await chanjetApi.get('/api/goodsIssue/detail', { id });
        // await processOrder(detail);
    }
    return true;
});
```

### 3. ACK、断线重连与幂等处理

- **ACK (确认机制)**：在处理器中 `return true;`，SDK 会自动构造并发送 `sys_ack` 帧给网关，确认消息已消费。若抛出异常或 `return false;`，则不会发送 ACK。
- **断线重连**：SDK 内置了心跳保活机制和指数退避（Exponential Backoff）重连逻辑。网络波动导致的断开会自动重新连接，无需人工干预。
- **幂等处理**：由于消息保证“至少投递一次”（At-Least-Once），可能存在重复推送。请务必使用 `msg.id` 进行去重。

```javascript
dispatcher.onAppNotice('GoodsIssue', async (msg) => {
    const msgId = msg.id; // 全局唯一消息ID
    
    // 伪代码：在 Redis 或 DB 中检查是否已经处理过
    // if (await isProcessed(msgId)) return true; // 已处理过直接回复 ACK 即可
    
    // 执行业务逻辑...
    
    // await markAsProcessed(msgId);
    return true;
});
```

## 推送可靠性机制

为了保证企业数据的安全与完整，Stream Gateway 提供了以下可靠性保证：

1. **至少一次投递 (At-Least-Once)**：网关保证消息至少成功投递一次。如果客户端未在指定时间内返回 ACK，或者连接意外断开，网关会在下次连接或超时后重新推送该消息。
2. **重复推送与幂等**：基于“至少一次”的策略，在网络超时或客户端处理过慢时，有可能会收到重复的消息。**接入方必须实现消息幂等**，建议使用消息自带的 `id` (Message ID) 作为唯一键进行防重。
3. **断线重连与心跳**：SDK 会与网关保持 WebSocket 长连接，并维持心跳。当超过心跳周期无响应或网络断开时，SDK 会自动发起带有随机抖动的指数退避重连（避免大量客户端同时重连导致服务端雪崩）。
4. **ACK 格式**：对于上层业务透明，当业务回调函数成功执行并返回 `true`，SDK 会自动向网关发回类型为 `sys_ack` 的控制帧，告知该消息已被签收。
5. **漏单补偿策略**：
   - **死信队列 (DLQ)**：SDK 提供死信机制，对于消费抛错、处理失败的消息，可以转储至本地或 Redis 暂存，避免阻塞后续消息处理，待问题修复后重新投递。
   - **定时对账拉取 (推荐)**：强烈建议业务系统配备一个定时补偿任务。每日调用开放平台的“单据列表查询” API，通过修改时间区间 (`modified_time`) 主动对比、拉取变更数据，作为解决极端网络故障或长时期宕机导致的漏单兜底机制。
## 许可证

MIT

## 更新日志 (Changelog)

### v0.2.0 (2026-06)
- **新增**: 死信队列 (DLQ) 机制，支持通过配置 `DlqProvider` 接口暂存处理异常的消息，防止发生漏单。
- **新增**: 断线重连升级为指数退避 (Exponential Backoff) 与随机抖动重连，并开放自定义 `maxBackoff` 和 `reconnectInterval` 配置项。
- **新增**: 增强 `MessageDispatcher` 路由，增加兜底处理器 `setFallbackHandler`，以及专属 `onOrderStatus` 与 `onAppNotice` 宏方法。
- **优化**: 增强解密引擎鲁棒性，自动净化（Sanitize）传入秘钥中可能包含的不可见控制字符，并对齐兼容 32 位 Hex 秘钥。
