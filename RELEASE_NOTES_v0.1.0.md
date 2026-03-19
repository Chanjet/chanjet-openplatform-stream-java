# 畅捷通 Stream Gateway 交付文档 v0.1.0 (Release Notes)

## 1. 项目概述 (Overview)
畅捷通 Stream Gateway 是一个高性能、低延迟的 Webhook-to-WebSocket 同步桥接器。它允许 ISV 在无公网 IP 和 SSL 证书的环境下，通过 WebSocket 长连接安全、实时地接收来自畅捷通核心服务的业务事件。

---

## 2. 技术栈 (Tech Stack)
- **Runtime**: JDK 21 (强制要求，基于虚拟线程优化)
- **Framework**: Spring Boot 3.2.4+ / Spring Framework 6.1+
- **Storage**: Redis Cluster (用于分布式路由与 Nonce 管理)
- **Registry**: Nacos (微服务发现与配置)
- **SDK**: Java 21 HttpClient-based

---

## 3. 部署配置指南 (Deployment Guide)

### 3.1 环境要求
- 推荐使用 **GraalVM JDK 21** 以获得最佳性能。
- 确保 Redis 集群可用。
- 确保 Nacos 注册中心已启动。

### 3.2 核心配置 (`application.yml`)
```yaml
spring:
  threads:
    virtual:
      enabled: true  # 必须开启，以支持高并发阻塞 I/O

connector:
  node-id: ${spring.cloud.client.ip-address}:${server.port} # 节点在集群中的唯一标识

services:
  auth:
    id: cjt-auth-service        # 提供签名验证能力的微服务名
  subscription:
    id: cjt-subscription-manager # 提供推送状态控制能力的微服务名

# Redis 配置 (标准的 Spring Data Redis 配置)
spring.data.redis:
  cluster:
    nodes: 127.0.0.1:6379,127.0.0.1:6380
```

### 3.3 部署运行
```bash
# 编译
mvn clean install -DskipTests

# 运行网关节点
java -jar connector-server/target/connector-server-0.1.0-SNAPSHOT.jar
```

---

## 4. ISV Java SDK 使用说明 (SDK Usage)

### 4.1 引入依赖
```xml
<dependency>
    <groupId>com.chanjet.connector</groupId>
    <artifactId>connector-sdk-java</artifactId>
    <version>0.1.0-SNAPSHOT</version>
</dependency>
```

### 4.2 快速开始
```java
// 1. 初始化客户端
GatewayClient client = GatewayClient.builder()
        .appKey("your-app-key")
        .appSecret("your-app-secret")
        .gatewayUrl("http://gw-host:8080") // 网关 HTTP 地址
        .build();

// 2. 注册业务处理器
client.onEvent(frame -> {
    System.out.println("收到消息: " + frame.payload());
    // 返回 true 后，SDK 会自动发回 200 ACK 给网关，网关随后响应 Core
    return true; 
});

// 3. 启动连接 (内部会自动处理 Nonce 挑战与签名计算)
client.start();
```

---

## 5. 开发与验证 (Verification)

### 5.1 运行全链路 TCK
TCK (Technology Compatibility Kit) 验证了从 Webhook 接收到 SDK 推送的完整闭环。
```bash
cd services/gateway-java/connector-server
mvn test -Dtest=TckIntegrationTest
```

### 5.2 核心设计规范
- **精准寻址**: 补丁 `SYKFPT-1061-Patch-P2P` 确保了多连接场景下的消息定向。
- **背压保护**: 默认单节点并发上限 5000，单租户并发上限 100。
- **自愈机制**: 30 分钟容忍期后自动进入推送挂起状态。

---
**版本状态**: v0.1.0-Stable
**整理日期**: 2026-03-19
