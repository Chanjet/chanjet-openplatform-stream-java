# 核心回归测试用例集 (Regression Test Cases)

> **目标**：本文档记录了 Stream Gateway v0.1.0 在分布式环境下的核心边界场景，用于后续版本升级时的质量回归。

---

## 1. 基础设施 (Infrastructure)

### [RC-01] 注册中心与解密验证
- **场景**: 应用在 Profile=inte 下启动。
- **预期**: 
    - 日志显示 `spring.cloud.nacos.discovery.secret-key` 解密成功。
    - Actuator 端点显示 `nacosDiscovery` 状态为 `UP`。

---

## 2. 消息路由 (Routing & P2P)

### [RC-02] 本地优先单播 (Local-First)
- **场景**: App A 连在 Node 1，Webhook 请求发往 Node 1。
- **验证**: Node 1 应直接推送消息，不应产生跨节点的 P2P HTTP 请求。

### [RC-03] 跨节点单播重试 (Resilient P2P)
- **场景**: Redis 存在过期路由 Node 2 (已下线) 和有效路由 Node 1。Webhook 发往 Node 3。
- **验证**: Node 3 尝试转发 Node 2 失败后，应自动重试 Node 1 并投递成功。

### [RC-04] P2P 环路熔断 (Loop Prevention)
- **场景**: 模拟极端路由错误，消息从 Node 1 转发至 Node 2，但 Node 2 发现本地连接已失效。
- **验证**: Node 2 检测到 `X-GW-Hop-Count > 0`，应记录错误并停止转发，禁止将消息再次发回 Node 1 或其他节点。

---

## 3. 安全鉴权 (Security)

### [RC-05] 令牌滚动更新支持
- **场景**: 网关配置了两个 `internal-tokens` [New, Old]。
- **验证**: 
    - 携带 Old Token 的 P2P 请求应被接受。
    - 携带 New Token 的 P2P 请求应被接受。
    - 携带其他 Token 或不带 Token 的请求应返回 `401 Unauthorized`。

---

## 4. 健壮性 (Resilience)

### [RC-06] 熔断器介入 (Throttling)
- **场景**: 对单一 AppKey 发起超高频 Webhook 调用（超过配置阈值）。
- **验证**: 系统应返回 `200`（或熔断日志），但内部不再发起转发，保护下游连接。

---
**版本**: v0.1.0
**最后更新**: 2026-03-19
