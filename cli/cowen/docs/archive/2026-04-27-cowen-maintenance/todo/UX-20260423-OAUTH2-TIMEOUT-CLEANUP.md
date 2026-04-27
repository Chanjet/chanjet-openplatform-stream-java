# 体验改进: OAuth2 认证周期自清理机制 (UX-20260423-OAUTH2-TIMEOUT-CLEANUP)

## 1. 背景与问题 (Context & Problem)
当前的 OAuth2 登录流程中，系统会在 Vault 中记录中间状态（如 `pending_auth_session` 和 `captured_auth_code`）。
- 如果认证流程在生命周期内（目前为 5 分钟）未成功完成（例如：用户关闭了浏览器、回调监听超时、或中间环节出错），这些记录可能会残留在本地。
- 残留的中间状态虽然有过期校验，但在下一次尝试前如果不主动清理，可能会导致状态机混乱或不必要的诊断报警。

## 2. 改进方案 (Proposed Improvements)
在 `finalize_login` 或相关的生命周期管理模块中引入强一致的自清理逻辑：

### 2.1 超时清理
当 `finalize_login` 中的 `tokio::select!` 触发 5 分钟超时分支时，应显式调用 `session_manager.clear(profile)` 以清除挂起的会话记录。

### 2.2 失败清理
当监听到明确的失败信号（如 Authorization Rejected）或 Token 换取失败（Exchange Error）后，在退出前执行清理工作，确保“干净”地退出。

### 2.3 鉴权前置清理
在 `login` 开始准备创建新会话前，可以先尝试 `clear` 掉旧的、已过期的或残余的会话信息。

## 3. 预期效果 (Expected Outcome)
- 提高认证流程的鲁棒性。
- 确保本地状态与实际认证进度高度一致。
- 减少由于残留状态导致的“看似在认证中”的误导性提示。

---
> [!TIP]
> 建议在 `src/cmd/auth.rs` 的 `finalize_login` 结尾处增加统一的 `defer` 风格清理逻辑或在错误路径进行清理。
