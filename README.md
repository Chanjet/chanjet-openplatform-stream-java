# 畅捷通 Stream Gateway

[![Version](https://img.shields.io/badge/version-0.1.5-blue.svg)](RELEASE_NOTES_v0.1.0.md)
[![JDK](https://img.shields.io/badge/JDK-21-orange.svg)](https://openjdk.org/projects/jdk/21/)
[![Performance](https://img.shields.io/badge/Loom-Ready-brightgreen.svg)](https://openjdk.org/jeps/444)

畅捷通 Stream Gateway 是一个专为 ISV 设计的高性能 **Webhook-to-WebSocket** 透明同步桥接器。

---

## 🏗️ 核心架构
- **零信任 (No-Secret)**: 网关不持有 ISV 应用私钥，鉴权代理至核心服务。
- **并发引擎 (Virtual Threads)**: 基于 Java 21 虚拟线程，单节点支持海量长连接。
- **分布式寻址 (P2P Mesh)**: 内置跨节点消息中转，支持大规模集群部署。

---

## 🎯 业务支持场景
| 场景 | 解决方案 | 优势 |
| :--- | :--- | :--- |
| **内网环境接收事件** | Webhook 转 WebSocket 推送 | 免公网固定 IP，免 SSL 证书 |
| **集群多活部署** | 本地优先 + P2P 智能分发 | 降低延迟，保障消息触达率 |
| **高频流量冲击** | 双层令牌桶限流 + 熔断保护 | 保护 ISV 客户端不被流量淹没 |
| **全链路安全性** | 预校验 + 时效性 Nonce 签名 | 杜绝匿名攻击与重放攻击 |

---

## 🛠️ 快速起步

### 1. 构建
```bash
# 构建 Java 服务端
make build-java

# 构建 Rust 治理工具 (CLI)
cd cli/cowen && make macos-aarch64
```

### 2. 核心配置 (最小集)
```yaml
connector:
  node-id: 127.0.0.1:8080
  internal-tokens: ["your-p2p-token"]
spring.data.redis.host: localhost
```

### 3. 文档指引
- [详细配置字典指南](RELEASE_NOTES_v0.1.0.md#3-配置参考指南-configuration-reference)
- [安全签名算法说明](docs/prd/v0.1.0/websocket-auth-deliverables.md)
- [核心回归测试报告](docs/design/Regression_Test_Cases.md)

---
© 2026 畅捷通架构组
