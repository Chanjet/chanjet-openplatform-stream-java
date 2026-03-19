# OpenSpec 提案：WebSocket 会话管理 (SYKFPT-1061-5.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-5.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 5.1: WebSocket 会话管理 (WsHandler) |

---

## 1. 问题背景 (Context)
网关需要维持与成千上万 ISV 客户端的长连接。为了确保消息能够准确下发到特定客户端，并能够在连接失效时及时清理路由，接入层必须具备高性能的 WebSocket 会话管理能力、心跳保活机制以及与领域层（Core）的无缝集成。

## 2. 目标 (Objectives)
- 创建 `connector-server` Maven 子模块（Spring Boot 4）。
- 实现 `IConnectionManager` 契约，支持消息物理推送。
- 实现 `WsHandler` 处理 WebSocket 生命周期。
- 建立应用级心跳机制（10s Ping/Pong）。
- **严格遵循 TDD**：使用 `StandardWebSocketClient` 运行 WebSocket 集成测试。

## 3. 技术设计 (Technical Design)

### 3.1 会话注册表 (SessionRegistry)
- 维护本地内存中的 `Map<String, WebSocketSession>` (以 ClientID 为键)。
- 处理 Session 的建立 (`afterConnectionEstablished`) 与断开 (`afterConnectionClosed`)。

### 3.2 物理下发逻辑
- 实现 `push(clientId, frame)`: 从注册表中查找 Session 并执行 `sendMessage`。
- 处理发送失败：捕获 I/O 异常并返回 `false` 触发领域层失败处理逻辑。

### 3.3 心跳与自愈
- 定时器每 10 秒发送 `{"msg_type":"ping"}`。
- 20 秒未收到任何消息则强制断开物理连接并触发路由清理。

## 4. 实施计划 (Implementation Plan)
1.  **工程搭设**: 创建 `connector-server` 模块并配置 `pom.xml`（引入 WebSocket Starter）。
2.  **编写集成测试**: `WebSocketIntegrationTest`。启动本地服务器并模拟客户端连接。
3.  **编码实现**: 编写 `WsHandler`、`SessionRegistry` 及 `IConnectionManager` 的实现类。
4.  **状态同步**: 在建连/断连钩子中调用领域层 `handleReconnect` / `routeStore.remove`。

## 5. 验证策略 (Verification Strategy)
- **连通性验证**: 模拟客户端建连，验证 SessionRegistry 是否正确记录。
- **消息推送验证**: 通过 `IConnectionManager` 发送消息，验证客户端是否收到 JSON 帧。
- **自动断连验证**: 模拟心跳超时，验证物理连接是否被服务器主动关闭。

---
**审批意见**：待评审。
