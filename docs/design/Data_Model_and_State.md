# 畅捷通 Stream Gateway 数据模型与状态设计 v0.1.0

## 1. Redis 键设计 (Redis Schema)

网关集群通过 Redis 共享路由信息及鉴权状态。所有 Key 统一使用 `cjt:gw:` 前缀。

### 1.1 路由表 (Route Table)
- **Key**: `cjt:gw:route:{AppKey}`
- **Type**: `Set`
- **Value**: `{nodeId}:{clientId}`
- **示例**: `10.1.168.98:8080:p2p-test-client`
- **TTL**: 60s (由服务端心跳自动续期)
- **用途**: 支撑 P2P 跨节点消息转发与寻址。

### 1.2 鉴权挑战 (Nonce)
- **Key**: `cjt:gw:nonce:{UUID}`
- **Type**: `String`
- **Value**: `{AppKey}`
- **TTL**: 30s
- **用途**: 握手协议中的挑战令牌，单次有效。

### 1.3 首次失败计时器 (Fail Store)
- **Key**: `cjt:gw:fail_start:{AppKey}`
- **Type**: `String`
- **Value**: `{Timestamp}`
- **TTL**: 1h
- **用途**: 记录客户端全量离线后的首条消息到达时间，驱动 30 分钟容忍期自愈逻辑。

## 2. 状态机设计 (State Machine)

针对每个 AppKey，其在系统中的推送状态由 `ToleranceManager` 维护：

### 2.1 状态迁移说明
- **Online**: Redis 路由表不为空。网关执行分发。
- **InTolerance (30min)**: 路由为空且 Webhook 到达。网关返回 503，触发计时。
- **Suspended**: 超过 30 分钟无连接。网关通知 Core 停止向 Webhook 地址发送消息（挂载到离线池）。
- **Recovery**: 只要有任一客户端重连，立即通知 Core 恢复推送并补发积压消息。

## 3. 并发限流模型 (Resilience)

采用基于 **令牌桶 (Token Bucket)** 算法的内存级限流：

- **单节点最大并发**: 默认 5000。超过则返回 `503 NODE_OVERLOAD`。
- **单应用最大并发**: 默认 100。超过则返回 `429 TENANT_LIMITED`。

---
**更新日期**: 2026-03-19
