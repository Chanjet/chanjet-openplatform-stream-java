# Cowen CLI 0.2.0 新增 OAuth2 (PKCE) 认证模式需求文档

> [!WARNING]
> **本文档为早期草案，仅作为需求演进的历史参考。正式需求以 [PRD_v0.2.0_OAuth2_PKCE_Auth_Mode.md](./PRD_v0.2.0_OAuth2_PKCE_Auth_Mode.md) 为准。**

## 1. 背景与目的
为了支持更标准化的授权流程并满足部分不具备接收消息（AppTicket）能力的应用场景，`cowen` 0.2.0 版本计划引入基于 **OAuth 2.0 标准协议的 PKCE (Proof Key for Code Exchange) 模式**。该模式允许用户使用指定的 `client_id` (AppKey) 进行授权，并由 CLI 自动维护令牌（AccessToken 和 RefreshToken）的有效性。

## 2. 核心特征
- **标准协议支持**：遵循 RFC 6749 和 RFC 7636 (PKCE)。
- **内置 ClientID**：`client_id` (AppKey) 硬编码于二进制中，不可通过配置或参数更改，确保接入的一致性与安全性。
- **令牌轮换 (Rotation)**：支持 Refresh Token 轮换机制，每次刷新均持久化新令牌。
- **无消息模式**：此模式仅负责 API 调用鉴权维护，**不具备**接收及转发平台消息事件（Stream/Webhook）的能力。
- **配置隔离**：通过 `profile` 机制与原有的“自建应用模式”物理隔离。

## 3. 业务流程与交互设计

### 3.1 初始化 (init)
新增认证模式选择，**v0.2.0 起默认为 oauth2 模式**。
- **命令示例**：`cowen init` (等同于 `cowen init --mode oauth2`)
- **交互流程**：
    1. CLI 使用内置的 `client_id` 生成 `code_verifier` 并计算 `code_challenge`。
    2. 打印授权链接，引导用户在浏览器中打开并登录授权。
    3. 用户授权后获得 `code`。
    4. 用户将 `code` 输入回 CLI，CLI 调用 `/oauth2/token` 换取令牌对。
    5. 将 `access_token` 和 `refresh_token` 加密存储于 Vault。

### 3.2 令牌维护逻辑
- **主动检查**：在调用 API 前检查 AccessToken 有效期（保留 10% 提前过期逻辑）。
- **刷新操作**：AccessToken 过期时，使用 `refresh_token` 调用 `/oauth2/token` (grant_type=refresh_token)。
- **轮换更新**：必须将返回的**新** `refresh_token` 立即同步写入本地 Vault。
- **容错处理**：若刷新失败且返回 4007 (Token 不正确)，需提示用户重新执行 `init` 流程。

### 3.3 功能限制
- 当启用 `oauth2` 模式的 profile 时，`cowen daemon start` 命令将：
    - 仅启动本地逻辑维护（如日志、本地代理）。
    - **不建立** WebSocket 隧道。
    - **不接受** 云端消息推送。

## 4. 技术设计要点
- **存储增强**：`Vault` 需扩展支持存储 `refresh_token`。
- **PKCE 工具类**：实现基于 SHA-256 和 Base64URL 的 `code_challenge` 计算逻辑。
- **TokenPool 适配**：`VaultTokenPool` 需适配双令牌模式，确保在刷新阶段的并发安全性。

## 5. 影响评估
- **向下兼容性**：不影响原有的自建应用模式。
- **安全性**：`code_verifier` 仅在内存中使用，换取后销毁；`refresh_token` 必须加密存储。
- **扩展性**：为未来支持其他标准 OAuth2 模式（如 Implicit 模式）奠定基础。

---
> [!IMPORTANT]
> **开发约束**：
> 1. 本文档作为 0.2.0 阶段 PRD，需经人工审核通过。
> 2. 当前阶段严禁启动 OpenSpec 驱动及生产代码编写。
