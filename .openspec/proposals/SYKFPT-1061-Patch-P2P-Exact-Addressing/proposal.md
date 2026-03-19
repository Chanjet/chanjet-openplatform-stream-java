# OpenSpec 补丁提案：P2P 路由精度优化 (SYKFPT-1061-Patch-P2P)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-Patch-P2P |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **优先级** | 高 (High) |

---

## 1. 问题描述 (Problem)
在目前的 P2P 实现中，接收端（`WebhookController.receiveP2P`）采用 `appKey + "@local"` 的简易逻辑寻找本地 Session。
**缺陷：** 当 ISV 为同一个 AppKey 在同一个网关节点建立多个连接（如负载均衡、多活实例）时，目前的逻辑无法保证消息推送到 `MessageDispatcher` 选中的那个特定 `clientId`。

## 2. 解决方案 (Solution)
- **协议升级**: 在 `EventFrame` 中增加可选字段 `target_client_id`。
- **发送端增强**: `MessageDispatcher` 在进行 P2P 转发前，将选中的 `clientId` 填入 `EventFrame`。
- **接收端适配**: `WebhookController` 优先使用 `target_client_id` 进行本地推送。

## 3. 实施计划 (Implementation Plan)
1.  **Red (红)**: 更新 `TckIntegrationTest`，模拟一个 AppKey 对应两个连接的场景，验证 P2P 是否能推送到指定的 Client。
2.  **协议更新**: 修改 `connector-common` 中的 `EventFrame` Record。
3.  **逻辑重构**: 更新 `MessageDispatcher` 和 `WebhookController`。

## 4. 验证策略 (Verification Strategy)
- 运行集群模式 Mock 测试，确保即使有多实例存在，消息路径依然 100% 确定。
