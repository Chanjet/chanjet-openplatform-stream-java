# OpenSpec 提案：P2P 内部转发物理实现 (SYKFPT-1061-P2P)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-P2P |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关需求** | 实现多节点集群下的消息转发能力 |

---

## 1. 问题背景 (Context)
在网关集群部署模式下，Webhook HTTP 请求可能到达节点 A，但对应的 WebSocket 长连接却维持在节点 B。目前 `MessageDispatcher` 已具备识别目标节点的能力，但缺乏真正的网络转发手段。为了实现集群内的“寻址透明”，必须实现基于内部 HTTP 协议的 P2P 转发器。

## 2. 目标 (Objectives)
- 实现 `IP2PClient` 的具体类 `RestP2PClient`。
- 实现节点间的 HTTP 消息传递协议。
- 确保转发过程中 TraceId 和 MsgId 的透传。
- 支持基于 Java 21 虚拟线程的同步阻塞转发。
- **严格遵循 TDD**：使用 **WireMock** 模拟远程网关节点验证转发逻辑。

## 3. 技术设计 (Technical Design)

### 3.1 内部转发路径
- **URL**: `POST http://{target_node_ip}:{port}/internal/v1/p2p/push`
- **Body**: 完整的 `EventFrame` JSON。
- **Header**: 包含内部通讯令牌 `X-Internal-Secret`（可选，用于安全加固）。

### 3.2 转发逻辑流
1.  `MessageDispatcher` 发现目标在远程 Node。
2.  调用 `IP2PClient.forward(nodeId, frame)`。
3.  `RestP2PClient` 构造 POST 请求并同步执行。
4.  接收到远程响应后，返回执行结果（成功/失败）。

## 4. 实施计划 (Implementation Plan)
1.  **编写测试用例**: `RestP2PClientTest`。模拟远程节点接收并返回 200/503。
2.  **编码实现**: 在 `connector-infra` 中创建 `RestP2PClient` 类。
3.  **配置装配**: 在 `InfraConfig` 中替换原有的 Stub 实现。
4.  **接口暴露**: 在 `WebhookController` 中增加对内部 P2P 路径的处理。

## 5. 验证策略 (Verification Strategy)
- **转发准确性**: 验证发送到 Node B 的数据包 Header 与 Node A 接收到的完全一致。
- **超时保护**: 验证当 Node B 无响应时，Node A 能在 3s 内快速失败并释放资源。

---
**审批意见**：待评审。
