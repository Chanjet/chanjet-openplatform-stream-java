# Specification Delta: OAuth2 Session Cleanup

## ADDED Requirements

### Requirement: 认证会话自清理
WHEN OAuth2 认证流程触发超时、失败或被用户取消时,
系统 SHALL 自动清除本地存储的所有中间认证状态。

#### Scenario: 超时自动清理
GIVEN 存在一个处于 "Pending" 状态的认证会话
WHEN 5 分钟超时触发
THEN 系统 SHALL 删除 Vault 中的 "pending_auth_session" 记录。

#### Scenario: 登录前置清理
GIVEN 存在上一次残留的已过期会话记录
WHEN 用户执行 `login` 命令
THEN 系统 SHALL 在发起新请求前清除旧的会话记录。
