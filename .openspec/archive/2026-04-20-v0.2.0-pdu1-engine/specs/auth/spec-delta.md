# Specification Delta: Cowen CLI v0.2.0 PDU 1 - OAuth2 Protocol Engine

## ADDED Requirements

### Requirement: PKCE 协议支持 (PKCE Protocol Support)
系统 SHALL 支持标准 PKCE (RFC 7636) 流程,
Verifier 为 64 字节随机字符串, Challenge 使用 S256 算法。

### Requirement: 并发刷新锁 (Concurrent Refresh Lock)
系统 SHALL 在发起网络令牌刷新请求前获取 Profile 级文件排他锁,
并在锁内执行 Double-Check 以防止重复刷新。

### Requirement: 令牌自动轮换 (Token Rotation)
每次通过 `refresh_token` 换取新令牌成功后,
系统 SHALL 自动将响应中的新 `refresh_token` 持久化至 Vault。

### Requirement: 宽限期弹性处理 (Grace Period Resilience)
WHEN 刷新请求并行发生且平台处于宽限期 (5min) 时,
系统 SHALL 能够接受重复的令牌对而不视为故障。

## MODIFIED Requirements

### Requirement: OAuth2Provider 行为 (AuthProvider Impl)
系统 SHALL 实现 `OAuth2Provider`,
对接 `https://openapi.chanjet.com/oauth2/token` 端点。

#### Scenario: 令牌过期自动重整
GIVEN 系统检测到 Access Token 过期
WHEN 调用 `get_token`
THEN 系统 SHALL 自动发起 Refresh Grant 请求
AND 成功后更新本地缓存与 Vault。

#### Scenario: 刷新完全失效
GIVEN Refresh Token 已过期 (4029) 或无效 (4007)
WHEN 尝试刷新
THEN 系统 SHALL 抛出会话失效异常, 引导用户执行 `owenc init`。
