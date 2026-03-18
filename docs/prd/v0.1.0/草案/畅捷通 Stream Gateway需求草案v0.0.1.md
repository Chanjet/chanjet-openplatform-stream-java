# 畅捷通 Stream Gateway需求草案v0.0.1

这是一份基于我们所有深入讨论后整理出的**完整产品与技术需求文档 (PRD / Architecture RFC)**。您可以直接将此文档作为研发立项、架构评审或团队开发的标准依据。

---

# 📄 畅捷通 Stream Gateway (Webhook-to-WebSocket) 架构设计与需求文档

## 1. 背景与目标 (Background & Objectives)

### 1.1 业务痛点

目前畅捷通开放平台仅支持标准的 HTTP Webhook 事件推送。这给 ISV（独立软件开发商）和本地化部署的客户端（如 OpenClaw Agent、本地 ERP 插件）带来了极大的接入阻力：

1.  **网络门槛高**：必须具备公网 IP 或域名，且需配置 SSL 证书。
    
2.  **安全风险大**：开放公网端口容易遭受恶意扫描和攻击。
    
3.  **调试成本高**：本地开发环境无法直接接收外网 Webhook，需依赖内网穿透工具。
    

### 1.2 产品目标

开发一个官方的 **Chanjet Stream Gateway** 组件。该组件作为畅捷通 Core 与外部客户端之间的“透明桥梁”，将 HTTP Webhook 实时转换为 WebSocket 长连接推送。

*   **对畅捷通 Core**：零代码侵入，Core 依然认为是向一个普通的 URL 发送 Webhook。
    
*   **对 ISV 客户端**：无需公网 IP，只需主动发起 WebSocket 连接即可安全、实时地接收业务事件。
    

---

## 2. 核心架构原则 (Core Principles)

本组件的设计严格遵循以下四大原则：

1.  **零信任与透明转发 (Zero Trust & Transparent)**：网关**绝对不接触、不存储、不解密**业务明文数据。网关仅负责搬运原始 HTTP 报文（Headers + Raw Body）。`AppSecret` 永远不离开客户端本地，验签逻辑由客户端完成。
    
2.  **绝对无状态 (Stateless Proxy)**：网关内部**不设消息队列，不持久化业务数据**。消息的可靠性投递完全依赖畅捷通 Core 原生的 Webhook 衰减重试机制。
    
3.  **去中心化路由 (Decentralized Routing)**：不依赖 L7 负载均衡（如 Nginx 一致性哈希），网关集群通过轻量级 Redis 注册表实现节点间的 P2P 动态路由。
    
4.  **极速自愈 (Fast Self-Healing)**：摒弃传统的 TTL 等待机制，采用“网关主动剔除”与“客户端高频心跳”结合，实现毫秒级故障转移。
    

---

## 3. 系统架构与核心流转 (System Architecture)

### 3.1 拓扑结构

```text
[畅捷通 Core] --(POST Webhook)--> [公网 LB] --(随机分发)--> [Gateway Node B]
                                                               │ (查 Redis 路由)
                                                               │ (内部 P2P HTTP 转发)
                                                               ▼
[ISV 客户端] <--(WebSocket 推送 & ACK)--------------------- [Gateway Node A]

```

### 3.2 核心流转时序 (同步阻塞模式)

1.  **建连与注册**：客户端连入 Node A，Node A 在 Redis 写入路由 `AppKey_123 -> Node_A_IP`。
    
2.  **接收与寻址**：Core 发送 Webhook 被随机打到 Node B。Node B 查 Redis 得知目标在 Node A。
    
3.  **内部转发**：Node B 向 Node A 发起内部 HTTP 请求，并**挂起等待**。
    
4.  **下发与确认**：Node A 将报文通过 WS 推给客户端，并**挂起等待**。客户端本地验签处理后，通过 WS 返回 `ACK`。
    
5.  **链路释放**：Node A 收到 ACK -> 响应 Node B (HTTP 200) -> Node B 响应 Core (HTTP 200)。
    
6.  **异常重试**：链路中任何一环超时或断开，Node B 均向 Core 返回 `503/504`，触发 Core 的原生衰减重试。
    

---

## 4. 详细功能设计 (Detailed Design)

### 4.1 客户端鉴权与安全建连

*   **连接地址**：`wss://stream.chanjet.com/ws/v1/events`
    
*   **鉴权方式**：URL Query 签名鉴权。
    
    *   参数：`app_key`, `timestamp`, `sign`
        
    *   算法：`sign = HMAC_SHA256(app_key + timestamp, AppSecret)`
        
*   **防重放**：网关校验 `timestamp` 与服务器时间差不得超过 5 分钟。
    
*   **网关动作**：网关通过内部接口获取 `AppSecret` 进行签名比对。验证通过后升级为 WebSocket，并向 Redis 注册路由。
    

### 4.2 灰度兼容与 URL 自动校验

*   ISV 在畅捷通后台配置统一的网关接收地址：`https://stream.chanjet.com/relay/v1/{AppKey}`
    
*   **自动过审**：当 Core 发起 `GET` 校验请求时，网关直接提取 Query 中的 `check_code` 并返回 `200 OK`，实现免配置自动校验。
    

### 4.3 动态路由与极速自愈 (Self-Healing)

*   **路由注册**：Node 节点接受 WS 连接后，在 Redis 写入 `SETEX route:{AppKey} 60 {Node_Internal_IP}`，并每 30 秒心跳续期。
    
*   **见死即埋 (Active Eviction)**：当 Node B 向 Node A 发起内部转发时，如果捕获到 `ECONNREFUSED` 或极短超时（如 1.5s），Node B **立刻删除 Redis 中的脏路由**，并向 Core 返回 503。
    
*   **客户端心跳**：客户端每 10 秒发送 WS Ping，若 20 秒未收到 Pong，客户端主动断开并重连，重连后强制覆盖 Redis 路由。
    

### 4.4 优雅降级与背压机制 (Backpressure)

采用\*\*“微观靠内存，宏观靠 Redis”\*\*的混合策略：

1.  **节点级全局保护 (内存)**：单台 Node 限制最大挂起 HTTP 请求数（如 5000）。超限直接返回 503。
    
2.  **租户级并发限制 (内存)**：单个 `AppKey` 在单台 Node 上限制最大并发挂起数（如 100）。超限返回 429，防止“吵闹的邻居”耗尽节点资源。
    
3.  **宏观熔断器 (Redis)**：若某 `AppKey` 连续失败/超时 50 次，Node 向 Redis 写入熔断标记（TTL 60s）。网关最外层入口检测到熔断标记，直接拦截并返回 503，保护集群免受无效流量冲击。
    

---

## 5. 接口与协议规范 (Protocol Specifications)

### 5.1 透明下发报文格式 (Gateway -> Client)

网关通过 WebSocket 下发的 JSON 必须包含原始签名和未被解析的 Body 字符串。

```json
{
  "req_id": "gw_1710676800_abc12",
  "type": "webhook_forward",
  "payload": {
    "headers": {
      "x-chanjet-signature": "v2:abcdefg...",
      "content-type": "application/json"
    },
    "body": "{\"encrypted_data\":\"...\"}" // 必须是原始字符串
  }
}

```

### 5.2 客户端回执格式 (Client -> Gateway)

客户端处理完毕后，必须在超时时间（如 4 秒）内返回 ACK。

```json
{
  "action": "ack",
  "req_id": "gw_1710676800_abc12"
}

```
---

## 6. 部署与运维要求 (Operations)

1.  **无状态部署**：Gateway Node 节点完全无状态，支持 Docker/K8s 随时横向扩缩容。
    
2.  **内部网络互通**：集群内的 Node 节点必须在同一 VPC 内，且互相开放内部转发端口（如 9001）。
    
3.  **Redis 依赖**：仅需极低配置的 Redis 实例（仅用于存储极少量的路由 String 和熔断标记，无高频读写）。
    
4.  **可观测性**：必须记录结构化日志，包含 `trace_id`, `app_key`, `latency_ms`, `status`，以便追踪消息生命周期。
    

---

## 7. 客户端 (ISV) 接入须知

1.  **幂等性要求**：由于网络波动或超时，畅捷通 Core 可能会触发重试。客户端**必须**基于业务单据号或 Event-ID 实现业务处理的幂等性。
    
2.  **验签责任**：客户端必须使用本地配置的 `AppSecret` 对收到的 `payload.body` 进行 HMAC-SHA256 验签，比对 `headers['x-chanjet-signature']`。
    
3.  **快速响应**：客户端应尽量异步处理耗时业务，确保在 3 秒内向网关返回 ACK，避免触发上游超时重试。