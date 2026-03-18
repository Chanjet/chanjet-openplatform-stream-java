# OpenSpec 提案：背压与熔断控制 (SYKFPT-1061-3.3)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-3.3 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 3.3: 背压与熔断控制 (Resilience) |

---

## 1. 问题背景 (Context)
作为一个 Webhook-to-WebSocket 的桥接器，网关的内存和线程资源受限于正在处理的“挂起”请求数。如果某个 ISV 客户端处理缓慢或遭遇流量洪峰，可能会导致网关节点资源耗尽，进而影响其他健康的租户。因此，必须引入背压（Backpressure）和熔断机制。

## 2. 目标 (Objectives)
- 实现节点级最大并发请求限制（内存保护）。
- 实现租户级并发请求限制（防止大户干扰）。
- 实现基于失败率的简易熔断逻辑。
- **严格遵循 TDD**：编写并发和限流测试用例。

## 3. 技术设计 (Technical Design)

### 3.1 核心防护层
1.  **节点级背压 (Node-level)**: 使用全局计数器，超过阈值（如 5000）直接返回 503。
2.  **租户级限流 (Tenant-level)**: 针对每个 AppKey 维护并发计数器，超过阈值（如 100）返回 429。
3.  **熔断器 (Circuit Breaker)**: 监控每个 AppKey 的转发失败率，达到阈值（如 50%）后进入熔断状态，拦截该应用的所有请求 60s。

### 3.2 实现策略
- 利用 `AtomicInteger` 或 `Semaphore` 实现本地内存级计数。
- 通过装饰器模式或拦截器（AOP）将防护逻辑注入 `MessageDispatcher`。

## 4. 实施计划 (Implementation Plan)
1.  **编写测试用例**: `ResilienceTest`。模拟高并发请求和部分请求失败。
2.  **实现 `ResilienceManager`**: 提供 `tryAcquire` 和 `release` 接口。
3.  **集成**: 在 `MessageDispatcher` 入口处应用防护逻辑。

## 5. 验证策略 (Verification Strategy)
- **限流验证**: 模拟超过阈值的并发请求，验证是否返回了预期的拒绝状态。
- **熔断验证**: 模拟连续失败，验证后续请求是否被直接拦截。

---
**审批意见**：待评审。
