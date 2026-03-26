# Proposal: 开放平台配套改造需求 (PR) for v0.1.1

## Why
`cjtCli` v0.1.1 引入了自动化的 AppTicket 保活机制和基于 Method+Path 的透明代理能力。为了实现这些功能，开放平台必须提供标准化的 Token 获取、Ticket 强制触发以及 WebSocket 握手挑战接口。

## What Changes
- 在 `docs/v0.1.1/prd/` 目录下新增《开放平台配套改造需求 (PR).md》。
- 明确四个关键接口的路径、入参、响应及安全规范：
    1. `getAppAccessToken`: 基础鉴权凭证获取。
    2. `triggerAppTicketPush`: 冷启动时的主动寻票。
    3. `challenge`: WebSocket 握手 Nonce 申请。
    4. `connect`: WebSocket 长连接建立。

## Impact
- **Specs**: 补充了 v0.1.1 版本的外部依赖契约。
- **Collaboration**: 为开放平台后端开发提供明确的验收基准。
