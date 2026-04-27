# Proposal: Webhook 回环地址强制限制 (SEC-20260423)

## Why
目前 Webhook 监听器和 Proxy 监听器虽然默认使用 `127.0.0.1`，但缺乏强制性的逻辑校验。如果未来由于配置错误或代码重构导致监听在公网或局域网 IP，会产生严重的安全风险，包括凭据泄露和未授权访问。

## What Changes
1. 在 `src/core/network.rs` 中新增 `validate_loopback_addr` 校验算子。
2. 在 `src/daemon/proxy.rs` 的 `start_proxy` 中引入该校验。
3. 在 `src/auth/lifecycle/listener.rs` 的 `OAuth2CallbackListener::start` 中引入该校验。
4. 确保所有监听在非法地址时抛出明确的 `SecurityError` 并安全退出。

## Impact
- **Security**: 提升了本地监听服务的防御能力，防止意外暴露。
- **Developer**: 提供了标准化的回环地址校验工具函数。
- **User**: 无感变更，除非用户尝试通过非法手段修改监听地址。
