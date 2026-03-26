# 开放平台配套改造需求 (PR) - v0.1.1

| 维度 | 内容 |
| --- | --- |
| **文档状态** | 正式版 (配合 cjtCli v0.1.1) |
| **生效版本** | v0.1.1 |
| **核心协作项** | AppTicket 主动触发；OpenAPI 元数据抓取 |

---

## 1. 外部接口依赖 (Existing Dependencies)
`cjtCli` 依赖以下开放平台现有接口进行基础鉴权：

### 1.1 获取应用 AccessToken (getAppAccessToken)
- **接口路径**: `POST /auth/appAuth/getAppAccessToken`
- **文档参考**: [自建应用授权说明](https://open.chanjet.com/docs/file/apiFile/common/selfBuiltApp/selfBuiltAppAuth?id=32086)
- **说明**: 用于 CLI 在持有有效 Ticket 时换取业务调用凭证。

---

## 2. 平台侧新增/改造需求 (New Requirements)
为实现 v0.1.1 的高级特性（寻票自愈、语义检索），需平台方配合开发以下接口：

### 2.1 强制触发 AppTicket 推送 (triggerAppTicketPush)
- **接口路径**: `POST /auth/appAuth/triggerAppTicketPush`
- **请求体 (JSON)**: `{"appKey": "string"}`
- **说明**: **核心新增需求**。支持 CLI 冷启动。平台收到请求后，需立即向该 AppKey 异步推送最新的 `APP_TICKET` 事件（通过现有推送通道或 WebSocket 隧道）。

### 2.2 获取 OpenAPI 3.0 规范文件 (getOpenApiSpec)
- **接口路径**: `GET /metadata/v1/openapi/spec?app_key={AppKey}`
- **说明**: 用于 CLI 构建本地 Trie-Tree 路由引擎及进行入参自动校验。返回标准 OpenAPI 3.0+ JSON。

### 2.3 获取 API 摘要列表 (getApiList)
- **接口路径**: `GET /metadata/v1/openapi/list?app_key={AppKey}`
- **说明**: 用于 CLI 构建轻量级本地向量索引，支持 `--search` 语义化检索。

---

## 3. 安全与可用性约束
1. **触发限流**: `triggerAppTicketPush` 建议 1 分钟内同一 AppKey 仅允许触发一次。
2. **数据一致性**: `getApiList` 返回的接口列表应与该应用在后台勾选的权限范围保持一致。
3. **推送延迟**: 收到 `triggerAppTicketPush` 后，Ticket 下发延迟应 < 3s。

---
**更新日期**: 2026-03-26
