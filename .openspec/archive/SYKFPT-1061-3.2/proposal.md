# OpenSpec 提案：30 分钟容忍期状态机 (SYKFPT-1061-3.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-3.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 3.2: 30 分钟容忍期状态机 (Tolerance Logic) |

---

## 1. 问题背景 (Context)
当某个 ISV 应用（AppKey）下的所有客户端实例全部断开连接时，网关无法继续推送 Webhook。为了保证消息不丢失，网关需要利用畅捷通 Core 原生的“衰减重试”和“离线积压池”机制。网关需在客户端断连后的 30 分钟内返回 503 触发 Core 重试；若超过 30 分钟仍未恢复，则主动通知 Core 挂起该应用的推送。

## 2. 目标 (Objectives)
- 实现 `connector-core` 中的 `ToleranceManager` 状态机。
- 维护 `fail_start:{AppKey}` 计时器（基于 Redis）。
- 实现 30 分钟容忍期判定逻辑。
- 联通 `IPushControl` SPI，向 Core 发送 `DISABLE/ENABLE` 指令。
- **严格遵循 TDD**：先编写状态转换测试，再实现逻辑。

## 3. 技术设计 (Technical Design)

### 3.1 状态迁移逻辑
1.  **Online -> Waiting**: 接收到 Webhook 但发现无在线连接。记录 `fail_start` 时间，向 Core 返回 503。
2.  **Waiting -> Waiting**: 容忍期内（< 30min）后续 Webhook 到达。持续返回 503。
3.  **Waiting -> Suspended**: 容忍期结束（>= 30min）Webhook 到达。调用 `IPushControl.setPushEnabled(false)`，清理计时器。
4.  **Suspended/Waiting -> Online**: 任一客户端重连成功。调用 `IPushControl.setPushEnabled(true)`，恢复推送并触发补发。

### 3.2 Redis 交互
- 使用 `SETNX` 确保只有第一条失败消息能初始化计时器。
- 计时器 TTL 设置为 1 小时，防止死键。

## 4. 实施计划 (Implementation Plan)
1.  **编写测试用例**: `ToleranceManagerTest`。模拟时间流逝和 SPI 接口调用。
2.  **实现 `ToleranceManager`**: 处理分发失败事件和客户端建连事件。
3.  **集成**: 在 `MessageDispatcher` 中埋入失败回调钩子。

## 5. 验证策略 (Verification Strategy)
- **时序验证**: 模拟 0min, 29min, 31min 三个时间点的 Webhook 到达行为。
- **自愈验证**: 模拟在 Suspended 状态下客户端重连，验证是否触发了 Enable 指令。

---
**审批意见**：待评审。
