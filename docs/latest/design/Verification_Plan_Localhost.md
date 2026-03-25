# 本地能力验证测试计划 (Profile: localhost)

> **测试目标**：验证在相关微服务 PR 部署后，网关在 `localhost` 环境下与 Nacos、Auth 及 Subscription 服务的集成正确性。

---

## 1. 预置条件 (Prerequisites)
- **本地中间件**: Redis 已启动（6379 端口）。
- **注册中心**: Nacos MSE 连通正常（C6356-inte 命名空间）。
- **微服务状态**: 
    - `cjt-auth-service` 已部署并提供 `/verify-preauth` 和 `/verify-sign` 接口。
    - `cjt-subscription-manager` 已部署并提供 `/push-status` 接口。
- **本地应用**: 执行过 `make build-java` 编译。

---

## 2. 测试场景流 (Test Scenarios)

### 场景一：Nacos 注册与自检 (Infrastructure)
1.  **执行**: `java -jar connector-server.jar --spring.profiles.active=localhost`。
2.  **验证**:
    - [ ] 查看日志，确认 `spring.cloud.nacos.discovery.secret-key` 已成功解密。
    - [ ] 访问 `http://localhost:8081/actuator/health`，确认 `nacosDiscovery` 状态为 `UP`。
    - [ ] 登录 Nacos 控制台，确认服务 `open-streaming-gateway` 注册成功。

### 场景二：No-Secret 握手流程 (Security & Auth)
1.  **申请 Nonce**: 
    - `GET http://localhost:8080/v1/ws/challenge?app_key={TEST_KEY}`
    - Header: `X-CJT-PreAuth: {VALID_HMAC_PREFIX}`
    - [ ] **预期**: 返回 `code: GW-0000` 并包含 `nonce`。
2.  **建立 WS 连接**:
    - 使用 `GatewayClient` SDK 或 `wsc` 工具连接。
    - `ws://localhost:8080/connect?app_key={TEST_KEY}&nonce={NONCE}&sign={SIGN}`
    - [ ] **预期**: 连接成功升级，日志显示 `Client connected and registered`。

### 场景三：端到端消息推送 (Webhook Bridge)
1.  **准备**: SDK 保持连接。
2.  **触发推送**: 向网关发起 Webhook 请求。
    - `POST http://localhost:8080/internal/v1/webhook/dispatch`
    - Header: `X-C-APP_KEY: {TEST_KEY}`, `X-MSG-ID: T1`
    - Body: `{"hello":"world"}`
3.  **验证**:
    - [ ] SDK 回调函数成功打印消息。
    - [ ] Webhook 接口返回 `200 OK`。

### 场景四：自愈与状态同步 (Self-Healing)
1.  **断连**: 停止所有本地 SDK 客户端。
2.  **触发**: 发送一条 Webhook 到网关。
    - [ ] **预期**: 网关返回 `503 Service Unavailable`。
    - [ ] **验证**: 查看日志，网关应向 Subscription 服务发送 `ENABLED=false` 指令（若超过容忍期）。
3.  **恢复**: 重新启动 SDK 客户端。
    - [ ] **验证**: 网关应向 Subscription 服务发送 `ENABLED=true` 指令。

---

## 3. 辅助验证指令
- **查看实时日志**: `tail -f gateway.log`
- **查看 Redis 路由**: `redis-cli SMEMBERS cjt:gw:route:{TEST_KEY}`
- **查看 Nacos 实例**: 使用 Nacos OpenAPI 或控制台。

---
**执行人**: @zhangliang
**计划日期**: 2026-03-19
