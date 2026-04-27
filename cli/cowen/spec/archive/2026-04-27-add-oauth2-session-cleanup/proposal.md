# Proposal: OAuth2 会话生命周期自清理 (UX-20260423)

## Why
目前的 OAuth2 流程中，如果认证超时或失败，中间状态（`pending_auth_session`, `captured_auth_code`）会残留在 Vault 中。这会导致状态机混乱，并在下次尝试登录时产生误导性的提示或潜在的冲突。

## What Changes
1. 在 `AuthSessionManager` 中新增 `clear(profile)` 方法，用于清理所有中间认证状态。
2. 在 `finalize_login` 函数的以下路径调用清理逻辑：
    - 超时 (Timeout) 分支。
    - 认证被拒绝 (Auth Rejected) 分支。
    - 交换令牌失败 (Exchange Error) 分支。
3. 在 `login` 函数入口处调用清理逻辑，确保每次开始都是“干净”的状态。

## Impact
- **UX**: 提高了认证流程的鲁棒性，减少残留状态导致的干扰。
- **Stability**: 确保本地状态与实际业务进度保持一致。
