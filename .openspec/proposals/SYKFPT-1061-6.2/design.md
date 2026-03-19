# Design: Technology Compatibility Kit (SYKFPT-1061-6.2)

## 1. 测试环境编排 (Topology)

TCK 将拉起一个完整的微缩环境：
- **Redis Container**: 真实的分布式路由中心。
- **Spring Boot Context**: 包含所有的 Core, Infra 实现。
- **Mock ISV App**: 使用 `connector-sdk-java` 建立长连接。
- **Webhook Client**: 模拟畅捷通 Core 发起 REST 推送。

## 2. 核心场景流 (Flows)

### 2.1 成功推送流 (TCK-01)
1. 启动 `GatewayClient` -> 握手成功。
2. 调用 `MockMvc.perform(post("/dispatch"))`。
3. `GatewayClient.onEvent` 被触发。
4. 验证 `GatewayClient` 的回调计数 +1。
5. 验证 `MockMvc` 返回 200 OK。

### 2.2 离线容忍流 (TCK-02)
1. `GatewayClient` 不启动。
2. 发起 Webhook Dispatch。
3. 验证返回 503。
4. 检查 Redis `fail_start` 键是否存在。

## 3. 辅助组件设计
- **`TckClient`**: 对 `GatewayClient` 的一层包装，提供同步等待消息到达的信号量（CountDownLatch）。
