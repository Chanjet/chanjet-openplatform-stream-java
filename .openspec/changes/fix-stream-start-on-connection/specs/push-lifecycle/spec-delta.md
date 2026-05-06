# 规范差异：连接生命周期 (Connection Lifecycle)

本文件包含对连接建立时行为的补充定义。

## MODIFIED 需求

### Requirement: WebSocket 连接副作用 (WebSocket Connection Side-effects)
**Previous**：系统仅在连接建立后注册本地会话和全局路由。

WHEN WebSocket 连接成功建立且包含有效的 appKey,
系统 SHALL 立即重置该 AppKey 的所有失败观察状态,
并通知 Core 服务恢复实时推送（如果此前已被挂起）。

#### Scenario: 恢复实时推送
GIVEN AppKey "app-1" 的推送状态为 SUSPENDED
WHEN 客户端 "client-1" 成功建立 WebSocket 连接并携带 appKey "app-1"
THEN 系统重置 "app-1" 的失败计时器
AND 系统调用 Core API 恢复 "app-1" 的推送
AND 系统在 Redis 路由表中注册 "app-1" -> "node-x:client-1"
