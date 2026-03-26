# Proposal: 增加 Go 语言版本 SDK (add-go-sdk)

## Why
目前项目已提供 Java 和 Node.js 版本的 SDK。为了支持 Go 语言技术栈的 ISV 能够低成本地接入畅捷通 Stream Gateway，我们需要提供一个功能对等、高性能且符合 Go 语言惯例的官方 SDK。

## What Changes
1. **新建 Go SDK 模块**: 在 `sdk/go` 目录下初始化 Go 项目。
2. **核心连接器 (GatewayClient)**: 实现基于 Nonce 的 HMAC 签名握手，并建立持久 WebSocket 连接。
3. **稳定性保障**: 实现带有指数退避 (Exponential Backoff) 的智能自动重连机制。
4. **安全解密 (CryptoUtils)**: 支持 AES-128-ECB 模式解密，支持独立加密密钥。
5. **消息分发器 (MessageDispatcher)**: 支持按 `msgType` 自动路由处理器，支持 `bizContent` 嵌套解析。
6. **示例程序**: 提供 `sdk/go-demo` 演示集成。

## Impact
- **规范**: 扩展 `docs/design/API_Specification.md` 以包含 Go SDK 的参考。
- **目录**: 新增 `sdk/go` 和 `sdk/go-demo`。
- **构建**: 更新顶层 `Makefile` 增加 Go 相关的测试指令。
- **用户**: 为 Go 开发者提供原生接入支持。
