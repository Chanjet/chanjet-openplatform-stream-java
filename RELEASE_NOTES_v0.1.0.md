# 畅捷通 Stream Gateway 交付文档 v0.1.0 (Release Notes)

## 1. 项目概述 (Overview)
畅捷通 Stream Gateway 是一个高性能、低延迟的 Webhook-to-WebSocket 透明同步桥接器。它解决了 ISV 在无公网 IP 和 SSL 证书环境下，实时、安全接收畅捷通核心服务（如 T+ Cloud, 畅捷通代账等）业务事件的需求。

---

## 2. 核心支持场景 (Supported Scenarios)

### 2.1 基础 Webhook 桥接 (Core Bridge)
- **场景**: ISV 部署在内网环境，无法提供公网回调 URL。
- **能力**: 网关作为公网代理接收 Webhook，并通过维护的长连接实时推送到 ISV SDK。

### 2.2 多端并发连接 (Multi-Instance Sync)
- **场景**: 同一 AppKey 拥有多个在线客户端（如不同标签页或集群节点）。
- **能力**: 网关支持多连接管理，确保每一条业务事件都能同步触达该 AppKey 下的所有活跃连接。

### 2.3 分布式集群与 P2P 转发 (Clustered P2P)
- **场景**: 网关多节点部署。Webhook 到达节点 A，但 ISV 连接在节点 B。
- **能力**: 节点 A 自动识别连接位置，通过内部 P2P 协议单播转发给节点 B，实现跨节点透明推送。

### 2.4 零停机滚动更新 (Smooth Token Rotation)
- **场景**: 集群需要更换内部通讯令牌。
- **能力**: 配置支持 `internal-tokens` 列表，新旧节点在切换期间可互信通讯，保障升级期间业务零中断。

### 2.5 节点故障自愈 (P2P Resilience)
- **场景**: 路由表存在过期节点或目标转发节点临时宕机。
- **能力**: 网关具备自动重试机制，转发失败时自动尝试集群内的备选路径。

---

## 3. 配置参考指南 (Configuration Reference)

| 配置项 | 说明 | 类型 | 默认值 | 必填 | 备注 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `server.port` | 业务服务监听端口 | Integer | 8080 | 否 | 用于 Webhook 和 WS 建连 |
| `management.server.port` | 管理端点监听端口 | Integer | 8081 | 否 | 用于健康检查，建议与业务隔离 |
| `connector.node-id` | 当前节点唯一标识 | String | 自动生成 | 否 | 格式 `ip:port`，K8s 下建议留空以使用自动发现 |
| `connector.internal-tokens` | 内部 P2P 认证令牌列表 | List | 无 | **是** | 第一个元素作为发送主令牌 |
| `services.auth.id` | 鉴权微服务名称 | String | 无 | 否 | Nacos 注册名。为空则进入 Mock 模式 |
| `services.subscription.id` | 订阅管理微服务名称 | String | 无 | 否 | 用于同步推送开启/挂起状态 |
| `spring.data.redis.*` | Redis 连接配置 | Object | - | **是** | 存储路由与 Nonce，支持集群/哨兵 |
| `spring.cloud.nacos.discovery.*` | Nacos 注册中心配置 | Object | - | 否 | 生产环境建议开启 |

---

## 4. 安全与鉴权指引 (Security & Auth)
详细算法请参考文档：[WebSocket 鉴权交付说明](docs/prd/v0.1.0/websocket-auth-deliverables.md)

---

## 5. 回归测试 (Quality Assurance)
核心边界场景已封装为自动化脚本，详见：[核心回归用例集](docs/design/Regression_Test_Cases.md)

---
**版本状态**: v0.1.0-Stable
**更新日期**: 2026-03-19
