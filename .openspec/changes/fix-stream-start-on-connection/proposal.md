# 提案：修复 WebSocket 接入后未启动实时推送的问题 (Fix Stream Start)

## Why

**背景**：
- 畅捷通 Stream Gateway 采用“容忍期自愈逻辑”。当 AppKey 的所有客户端离线超过 30 分钟时，网关会通知 Core 挂起实时推送。
- 当客户端重新连接时，网关必须立即通知 Core 恢复实时推送，以确保消息能及时送达。

**当前状态**：
- `DefaultWsHandler` 在 WebSocket 连接建立后（`afterConnectionEstablished`），仅注册了本地会话和 Redis 路由，但未调用 `ToleranceManager.resetFailureState`。
- 导致如果推送已被挂起，即使客户端连上，Core 也不会发送新的消息，除非该 AppKey 恰好有消息触发并走通了 `MessageDispatcher` 的逻辑（但如果推送已挂起，Core 根本不会发请求给网关）。

**期望状态**：
- 任何 WebSocket 客户端连接成功后，如果携带了 `appKey`，应立即重置该 AppKey 的失败状态，并通知 Core 恢复推送。

## What Changes

- 修改 `DefaultWsHandler.afterConnectionEstablished`：在连接成功后调用 `toleranceManager.resetFailureState(appKey)`。
- 增加单元测试验证该调用。

## Impact

### 受影响的规范
- 本次变更为对既有 PRD 逻辑的补全，不改变核心规范，但明确了连接建立时的副作用。

### 受受影响的代码
- `connector-server`: `DefaultWsHandler.java`
- `connector-server`: `DefaultWsHandlerUnitTest.java` (增加测试)

### 用户影响
- 修复了“连接后收不到消息”的严重 Bug。

### API 变更
- 无。

### 需要迁移
- [ ] 数据库迁移
- [ ] API 版本提升
- [ ] 用户沟通
- [ ] 文档更新

## 时间线评估
- 小 (1 小时内)

## 风险
- 无明显风险，该操作具有幂等性。
