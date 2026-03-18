# OpenSpec Project Context: 畅捷通 Stream Gateway

## 1. 项目愿景 (Vision)
为 ISV 提供一个高性能、低延迟、免公网 IP、免 SSL 证书的 Webhook-to-WebSocket 透明同步桥接基础设施。

## 2. 核心设计原则 (Guiding Principles)
- **Zero Trust (No-Secret)**: 网关不存储、不解密业务数据，AppSecret 仅存在于 ISV 本地。
- **Stateless Proxy**: 网关层不设消息队列，不持久化业务数据。
- **High Availability**: 集群化部署，支持跨节点 P2P 转发。
- **Self-Healing**: 毫秒级故障转移与自愈。

## 3. 技术栈 (Tech Stack)
- **Primary Runtime**: Java 21 (Virtual Threads)
- **Framework**: Spring Boot 4
- **Storage**: Redis Cluster
- **Communication**: WebSocket / Protobuf

## 4. 关键领域划分 (Domain Map)
- **Scaffolding**: Polyglot Monorepo 结构。
- **Contract**: SPI 驱动的逻辑与传输层分离。
- **Resilience**: 租户级背压控制。

---
**提示**：本文件为 OpenSpec 的全局上下文，所有实施提案均应参考此上下文。
