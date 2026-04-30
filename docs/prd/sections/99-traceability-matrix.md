# 需求溯源矩阵 (Traceability Matrix) {#99-traceability-matrix}

## 核心史诗核销表

| BRD Epic ID | 描述 | 关联 PRD 功能 (Features) | 状态 |
| :--- | :--- | :--- | :--- |
| **Epic-01** | 抽象化存储引擎 | [Feature-01], [Feature-02], [Feature-03] | `PASS` |
| **Epic-02** | 混合动力存储模式 | [Feature-05], [Feature-06] | `PASS` |
| **Epic-03** | ISV 全模式支持 | [Feature-07] | `PASS` |
| **Epic-04** | Proxy & Webhook 增强 | [Feature-08] | `PASS` |
| **Epic-05** | 历史资产继承 | [Feature-02] | `PASS` |

## 约束与边界核销

| 约束项 | 映射章节 | 备注 |
| :--- | :--- | :--- |
| 存储互斥性 | [04-业务规则 §1](./04-business-rules.md#RULE_STORAGE_MUTEX) | 物理锁定，严禁混用 |
| 消息透明转发 | [04-业务规则 §3](./04-business-rules.md#RULE_DISTRIBUTED_LOCK) | 仅去壳转发，业务系统处理幂等 |
| 历史兼容性 | [01-用户场景 §1](./01-introduction.md) | 默认保留 Legacy 模式 |

---
*核销完成，Phase 1.1 闭环。*
