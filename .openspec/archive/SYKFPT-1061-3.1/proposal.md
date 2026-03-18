# OpenSpec 提案：消息分发器逻辑 (SYKFPT-1061-3.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-3.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 3.1: 消息分发器逻辑 (Message Dispatcher) |

---

## 1. 问题背景 (Context)
网关系统接收到的 Webhook 需要根据注册在 Redis 中的路由信息推送到特定的 ISV 客户端。由于客户端可能连接在集群中的任意节点，消息分发逻辑必须能够区分“本地推送”与“跨节点转发”。

## 2. 目标 (Objectives)
- 实现 `connector-core` 中的 `MessageDispatcher` 核心逻辑。
- 对接 `IRouteStore` 获取路由信息。
- 对接 `IConnectionManager` 执行本地推送。
- 实现跨节点 P2P 转发的决策。
- **严格遵循 TDD**：先编写测试用例，再实现分发逻辑。

## 3. 技术设计 (Technical Design)

### 3.1 核心分发算法
1.  **输入**: 接收到 `EventFrame`。
2.  **寻址**: 调用 `IRouteStore.getNodes(appKey)` 获取目标节点列表。
3.  **决策**:
    - 如果目标节点包含当前节点 ID：调用 `IConnectionManager.push` 尝试本地下发。
    - 如果目标节点是远程节点：调用内部 `P2PClient` 进行转发。
    - 如果无在线节点：触发失败处理逻辑（Task 3.2 衔接）。
4.  **负载均衡**: 若同一个 AppKey 有多个在线连接，采用轮询 (Round-Robin) 或随机选择策略。

### 3.2 模块依赖
- 仅依赖 `connector-api` 定义的 SPI。
- 具体的 `nodeId` 通过系统环境变量或配置文件注入。

## 4. 实施计划 (Implementation Plan)
1.  **创建 `connector-core` 模块**: 配置 Maven 依赖。
2.  **编写测试用例**: `MessageDispatcherTest`。使用 Mockito 模拟 SPI 行为。
3.  **编写生产代码**: 实现分发逻辑，直到测试通过。
4.  **重构**: 优化性能或代码结构（如基于 Java 21 虚拟线程的并发转发）。

## 5. 验证策略 (Verification Strategy)
- **单元测试**: 覆盖本地命中、远程命中、离线三种核心场景。
- **并发测试**: 模拟高并发下的分发稳定性。

---
**审批意见**：待评审。
