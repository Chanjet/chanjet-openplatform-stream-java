# OpenSpec 提案：Java SDK 核心实现 (SYKFPT-1061-6.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-6.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 6.1: Java SDK 核心实现 |

---

## 1. 问题背景 (Context)
ISV 接入网关需要处理复杂的 WebSocket 握手（含 Nonce 挑战）、心跳保活、自动重连以及消息的签名验证。为了降低 ISV 的接入门槛并确保连接稳定性，官方需要提供一个生产级的 Java SDK。

## 2. 目标 (Objectives)
- 创建 `connector-sdk-java` 模块。
- 基于 Java 21 `HttpClient` 实现 WebSocket 客户端。
- 自动化握手流程：请求 Challenge -> 计算签名 -> 升级协议。
- 内置指数退避重连机制（Exponential Backoff）。
- 提供简单的事件回调接口（Listener/Callback）。
- **严格遵循 TDD**：使用 `Mock Server` 模拟网关行为验证 SDK。

## 3. 技术设计 (Technical Design)

### 3.1 核心组件
1.  **`GatewayClient`**: 主入口类，管理连接生命周期。
2.  **`ReconnectionStrategy`**: 实现基于指数退避的重连逻辑。
3.  **`MessageVerifier`**: 本地验签工具，防止伪造推送。

### 3.2 自动化流程
- **握手**: 自动调用 `GET /v1/ws/challenge` 获取 Nonce，然后发起 `wss://` 连接。
- **ACK**: SDK 捕获回调函数的执行结果，自动向网关发送 `AckFrame`。

## 4. 实施计划 (Implementation Plan)
1.  **工程搭设**: 创建 `sdk/java` 目录及 `pom.xml`。
2.  **编写测试用例**: `GatewayClientTest`。模拟握手失败、断连重连、消息验签。
3.  **编码实现**: 封装 `HttpClient` 的 WebSocket 接口。
4.  **示例代码**: 提供 `sample-java` 演示接入方式。

## 5. 验证策略 (Verification Strategy)
- **重连验证**: 强行断开服务端，观察 SDK 是否执行了预期的退避重连。
- **安全性验证**: 发送错误签名的消息，验证 SDK 是否正确拦截并拒绝触发业务回调。
- **并发验证**: 验证在虚拟线程环境下 SDK 的运行稳定性。

---
**审批意见**：待评审。
