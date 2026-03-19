# 畅捷通 Stream Gateway 交付文档 v0.1.0 (Release Notes)

## 1. 项目概述 (Overview)
畅捷通 Stream Gateway 是一个高性能、低延迟的 Webhook-to-WebSocket 同步桥接器。它允许 ISV 在无公网 IP 和 SSL 证书的环境下，通过 WebSocket 长连接安全、实时地接收来自畅捷通核心服务的业务事件。

---

## 2. 技术栈 (Tech Stack)
- **Runtime**: JDK 21 (强制要求，基于虚拟线程优化)
- **Framework**: Spring Boot 3.2.4+ / Spring Framework 6.1+
- **Storage**: Redis (用于分布式路由与 Nonce 管理)
- **Registry**: Nacos (微服务发现与配置)
- **SDK**: Java 21 HttpClient-based

---

## 3. 部署配置指南 (Deployment Guide)

### 3.1 环境要求
- 推荐使用 **GraalVM JDK 21** 以获得最佳并发性能。
- 确保 Redis (单机或集群) 可用。
- 确保 Nacos 注册中心已启动并完成服务注册。

### 3.2 注册中心与微服务配置 (`application.yml`)

#### 场景 A：使用 Nacos (推荐)
```yaml
spring:
  cloud:
    nacos:
      discovery:
        server-addr: 127.0.0.1:8848
        namespace: public

services:
  auth:
    id: cjt-auth-service        # 自动通过 Nacos 发现服务
  subscription:
    id: cjt-subscription-manager
```

#### 场景 B：不使用 Nacos (本地静态路由降级)
若内网环境无 Nacos，可关闭服务发现并手动指定物理地址：
```yaml
spring:
  cloud:
    discovery:
      enabled: false

services:
  auth:
    id: "" # 置空则不使用 ServiceId 路由
  subscription:
    id: ""

# 配合环境变量或启动参数直接指定 BaseUrl (需自定义 RestClient 配置)
```

### 3.3 Redis 存储详细配置
网关支持多种 Redis 拓扑结构：

**1. 单机或主从模式：**
```yaml
spring.data.redis:
  host: 127.0.0.1
  port: 6379
  password: your-password
```

**2. 哨兵模式 (Sentinel)：**
```yaml
spring.data.redis:
  sentinel:
    master: mymaster
    nodes: 10.0.0.1:26379,10.0.0.2:26379
```

**3. 集群模式 (Cluster)：**
```yaml
spring.data.redis:
  cluster:
    nodes: 10.0.0.1:6379,10.0.0.2:6379,10.0.0.3:6379
```

**4. Lettuce 连接池调优 (高并发建议)：**
```yaml
spring.data.redis:
  lettuce:
    pool:
      max-active: 200
      max-idle: 50
      min-idle: 10
      max-wait: 1000ms
```

**关键 Key 说明：**
- `cjt:gw:route:{appKey}` (Set): 存储节点物理寻址。
- `cjt:gw:nonce:{uuid}` (String): 握手挑战码。
- `cjt:gw:fail_start:{appKey}` (String): 容忍期计时器。

### 3.4 部署运行
```bash
# 1. 编译全量模块
mvn clean install -DskipTests

# 2. 运行网关节点
java -jar services/gateway-java/connector-server/target/connector-server-0.1.0-SNAPSHOT.jar
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
    return true; // 返回 true 自动发回 200 ACK
});

// 3. 启动连接
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

### 5.2 核心特性概览
- **精准寻址**: 补丁 `SYKFPT-1061-Patch-P2P` 确保了多连接场景下的消息定向。
- **背压保护**: 默认单节点并发上限 5000，单租户并发上限 100。
- **自愈机制**: 30 分钟容忍期后自动进入推送挂起状态。

---
**版本状态**: v0.1.0-Stable
**整理日期**: 2026-03-19
