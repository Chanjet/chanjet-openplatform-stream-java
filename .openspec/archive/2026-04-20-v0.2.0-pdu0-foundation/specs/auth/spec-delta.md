# Specification Delta: Cowen CLI v0.2.0 PDU 0 - Foundation

## ADDED Requirements

### Requirement: 认证模式配置 (Auth Mode Config)
WHEN 系统加载 Profile 配置,
系统 SHALL 支持 `app_mode` 字段,
其取值范围 SHALL 为 `self-built` 或 `oauth2`。

#### Scenario: 默认值兼容
GIVEN 存量 0.1.x 的配置文件（无 `app_mode` 字段）
WHEN 系统读取该配置
THEN `app_mode` SHALL 默认为 `self-built`。

### Requirement: 认证提供者抽象 (AuthProvider Abstraction)
系统 SHALL 通过 `AuthProvider` Trait 提供认证能力,
该接口 SHALL 支持 `get_token` 与 `refresh` 操作。

### Requirement: OAuth2 令牌存储模型 (OAuth2 Token Pair Model)
系统 SHALL 支持 `OAuth2TokenPair` 模型,
包含 `access_token`, `refresh_token`, `expires_at`, `refresh_expires_at` 字段。

## MODIFIED Requirements

### Requirement: 认证分发逻辑 (Auth Dispatching)
**更新前**: `AuthClient` 直接处理 AppTicket 换票逻辑。
**更新后**: `AuthClient` SHALL 根据当前 Profile 的 `app_mode`,
将令牌获取请求分发至对应的 `AuthProvider` 实例。

#### Scenario: 模式切换
GIVEN Profile 配置为 `app_mode: oauth2`
WHEN 调用 API 指令
THEN 系统 SHALL 调用 `OAuth2Provider`（本阶段为 Stub）发起令牌请求。
