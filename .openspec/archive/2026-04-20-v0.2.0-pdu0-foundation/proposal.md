# Proposal: Cowen CLI v0.2.0 PDU 0 - Foundation

## Why (驱动背景)
为了支持 OAuth2 (PKCE) 认证模式，Cowen CLI 需要一套可扩展的认证架构。当前 `AuthClient` 实现较为耦合，不支持多种认证模式的动态切换。本项目旨在引入 `AuthProvider` 抽象，并迁移存量的自建应用逻辑，为后续 OAuth2 引擎的接入奠定基准。

## What Changes (变更内容)
- **核心抽象**：
  - 定义 `AuthMode` 枚举（`SelfBuilt`, `OAuth2`）。
  - 在 `Config` 中新增 `app_mode` 字段。
  - 定义 `AuthProvider` Trait，提供统一的令牌获取与刷新接口。
- **存量迁移**：
  - 创建 `SelfBuiltProvider` 并实现 `AuthProvider`。
  - 将 `AuthClient` 中的核心逻辑（AppTicket 换票、重发 Ticket 等）迁移至 `SelfBuiltProvider`。
  - 重构 `AuthClient` 为路由器，根据 `Config` 选择 Provider。
- **数据模型**：
  - 定义 `OAuth2TokenPair`（用于存储 OAuth2 令牌）。
  - 定义 `AuthSession`（用于 OAuth2 授权中间状态）。
- **构建脚本**：
  - 更新 `build.rs` 以支持环境变量注入。

## Impact (影响范围)
- **规范**：新增认证模式与 Provider 抽象。
- **代码**：`src/auth/` 模块深度重构，保持 `Client` Trait 接口兼容。
- **用户**：无感知。存量 `self-built` 模式继续作为默认行为运行。
