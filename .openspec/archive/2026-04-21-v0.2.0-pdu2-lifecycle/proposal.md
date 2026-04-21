# Proposal: Cowen CLI v0.2.0 PDU 2 - Backend Finalizer & Lifecycle

## Why (驱动背景)
为了提供“零输入”的授权体验，CLI 需要在后台安静地处理 OAuth2 回调。由于主进程（owenc init）需要即刻退出以释放用户终端，必须引入一个脱离主进程生命周期的后台 Finalizer 进程，负责监听回调、换取令牌并清理会话状态。

## What Changes (变更内容)
- **后台 Finalizer**：
  - 实现基于 `axum` 的轻量级本地回调服务器，监听 `/callback` 路径。
  - 实现子进程脱离终端的实现（Detachment Logic）。
- **会话状态管理**：
  - 实现 `AuthSessionManager`，在 Vault 中持久化加密的待处理会话（PKCE Verifier, State, Port）。
- **接口集成**：
  - 实现 `auth login --finalize` 隐藏命令，作为 Finalizer 的执行入口。
- **自动清理**：
  - 监听器在处理完单个请求或 5 分钟超时后必须自动退出，并物理清理 Vault 中的 `pending_auth_session` 键。

## Impact (影响范围)
- **规范**：新增本地回调监听与会话捕获规范。
- **代码**：涉及 `src/auth/lifecycle/` 与 `src/cmd/auth.rs` 的隐藏逻辑。
- **用户体验**：实现了“执行即退出、授权后自动配置”的非阻塞流程。

## Verification Plan (验证计划)
- **测试**：验证后台进程启动、端口绑定、回调接收以及换票后的自毁逻辑。
- **环境隔离**：确保并发运行多个不同 Profile 的授权流时，端口与会话状态不冲突。
