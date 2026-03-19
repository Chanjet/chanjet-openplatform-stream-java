# 畅捷通 Stream Gateway 交付文档 v0.1.0 (Release Notes)

## 1. 项目概述 (Overview)
畅捷通 Stream Gateway 是一个高性能、低延迟的 Webhook-to-WebSocket 同步桥接器。它允许 ISV 在无公网 IP 和 SSL 证书的环境下，通过 WebSocket 长连接安全、实时地接收来自畅捷通核心服务的业务事件。

---

## 2. 技术栈 (Tech Stack)
- **Runtime**: JDK 21 (强制要求，基于虚拟线程优化)
- **Framework**: Spring Boot 3.2.4+ / Spring Framework 6.1+
- **Infrastructure**: Redis, Nacos MSE (注册中心), Spring Cloud LoadBalancer
- **Utility**: Chanjet Properties Encrypt (配置解密)
- **SDK**: Java 21 HttpClient-based

---

## 3. 部署配置指南 (Deployment Guide)

### 3.1 环境要求
- 推荐使用 **GraalVM JDK 21** 以获得最佳并发性能。
- 确保 Redis (单机或集群) 可用。
- 确保 Nacos MSE 实例可达。

### 3.2 核心配置与 Profile 切换
项目支持多环境配置，通过 `-Dspring.profiles.active` 切换：
- `localhost`: 本地开发环境（对接 Nacos MSE inte 命名空间）。
- `inte`: 集成测试环境。
- `moni`: 模拟演练环境。
- `prod`: 生产高可用环境。

### 3.3 Nacos MSE 配置 (以 localhost 为例)
本地开发环境下，`application-localhost.yml` 已配置连接至 MSE 注册中心：
```yaml
spring:
  cloud:
    nacos:
      discovery:
        server-addr: http://mse-a5fe1032-nacos-ans.mse.aliyuncs.com:8848
        namespace: C6356-inte
        group: OPEN
        access-key: ${NACOS_ACCESS_KEY} # 建议通过环境变量注入
        secret-key: ${NACOS_SECRET_KEY} # 支持加密格式，由插件自动解密
```

### 3.4 Redis 存储配置
支持单机、哨兵及集群模式。生产环境建议开启 Lettuce 连接池优化：
```yaml
spring.data.redis:
  lettuce:
    pool:
      max-active: 500
      max-wait: 2000ms
```

### 3.5 端口说明
- **8080 (业务端口)**: 用于 Webhook 接收 (`/internal/v1/webhook/dispatch`) 和 WebSocket 建连 (`/connect`)。
- **8081 (管理端口)**: 用于 Actuator 健康检查 (`/actuator/health`) 与监控。

---

## 4. 安全与鉴权指引 (Security & Auth)

网关采用 **No-Secret (零信任)** 架构，ISV 的 `AppSecret` 严禁离开其本地受控环境。

### 4.1 鉴权两阶段
1. **阶段一：Nonce 申请 (PreAuth)**
   - ISV 需在 Header 中携带 `X-CJT-PreAuth`。
   - **算法**: `HMAC_SHA256(app_key, AppSecret)` 的前 16 位小写十六进制。
   - **目的**: 验证应用合法性并防止 DDoS。

2. **阶段二：WebSocket 握手 (Sign)**
   - ISV 需在连接 URL 中携带 `nonce` 和 `sign`。
   - **算法**: `HMAC_SHA256(app_key + "&" + nonce, AppSecret)` 的完整小写十六进制。
   - **目的**: 确认连接请求持有者拥有对应应用的控制权。

---

## 5. ISV Java SDK 使用说明 (SDK Usage)

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
        .gatewayUrl("http://gw-host:8080")
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

## 5. 验证与运维 (Verification)

### 5.1 全链路 TCK 验证
```bash
make build-java
cd services/gateway-java/connector-server
mvn test -Dtest=TckIntegrationTest
```

### 5.2 健康检查
```bash
curl http://localhost:8081/actuator/health
```

---
**版本状态**: v0.1.0-Stable
**整理日期**: 2026-03-19
