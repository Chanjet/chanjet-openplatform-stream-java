# Design: WebSocket Session Management (SYKFPT-1061-5.1)

## 1. 物理架构与组件 (Components)

### 1.1 `WsSessionRegistry`
- **职责**: 管理本地活跃的 `WebSocketSession`。
- **存储**: `ConcurrentHashMap<String, WebSocketSession>`。
- **线程安全**: 确保在高并发建连/断连时的原子性。

### 1.2 `DefaultWsHandler`
- **继承**: `TextWebSocketHandler` (Spring WebSocket)。
- **核心逻辑**:
    - `afterConnectionEstablished`: 解析 Query Params (clientId)，存入 Registry。
    - `handleTextMessage`: 接收客户端心跳 (Pong) 或业务 ACK。
    - `afterConnectionClosed`: 从 Registry 移除，并调用领域层清理路由。

### 1.3 `LocalConnectionManager`
- **职责**: 实现 `IConnectionManager` 契约。
- **发送**: 查找 Registry -> `session.sendMessage(new TextMessage(json))`。

## 2. 心跳机制 (Heartbeat)
- **定时任务**: 使用 `@Scheduled(fixedRate = 10000)` 在 `WsSessionRegistry` 中遍历所有会话。
- **逻辑**: 发送 `{"msg_type":"ping"}` 并记录发送时间。
- **超时**: 在 `WsHandler` 中维护 `lastSeenTime`。若 `now - lastSeenTime > 20s`，则执行 `session.close()`。

## 3. TDD 集成测试计划
- **环境**: `@SpringBootTest(webEnvironment = RANDOM_PORT)`。
- **客户端**: `StandardWebSocketClient`。
- **用例**:
    - `shouldRegisterSessionOnConnect()`: 验证 Registry 包含新 clientId。
    - `shouldPushMessageToClientSuccessfully()`: 验证客户端收到预期 JSON。
    - `shouldCleanUpOnDisconnect()`: 验证断连后 Registry 为空。
