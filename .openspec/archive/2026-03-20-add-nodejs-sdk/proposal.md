# Proposal: 新增 Node.js SDK 支持

## Why
为了扩大畅捷通 Stream Gateway 的生态兼容性，方便 Node.js 技术栈的 ISV 接入，需要参照 Java SDK 的成熟逻辑，提供官方的 Node.js SDK。

## What Changes
1. **核心客户端 (`GatewayClient`)**: 
   - 实现基于 `ws` 库的 WebSocket 链接管理。
   - 实现 Nonce 预校验与握手签名算法（HMAC-SHA256）。
   - 实现智能重连策略（指数退避与随机抖动）。
   - 支持心跳维护（Ping/Pong）与消息确认（ACK）。
2. **业务分发器 (`MessageDispatcher`)**:
   - 实现 AES-256-CBC 业务消息解密逻辑。
   - 支持多级消息处理器注册（支持 `APP_NOTICE` 的语义化路由）。
3. **模型定义**:
   - 定义标准的 `EventFrame`、`AckFrame` 及常见业务消息模型（`BaseMessage` 等）。
4. **测试与演示**:
   - 提供单元测试覆盖核心逻辑。
   - 提供一个基础的使用示例。

## Impact
- **新增**: `sdk/nodejs/` 目录。
- **依赖**: 仅依赖 `ws` 和 `crypto`（内置）。
- **用户**: Node.js 开发者可以快速集成网关推送。
