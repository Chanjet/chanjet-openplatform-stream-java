# 畅捷通 Stream Gateway

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](RELEASE_NOTES_v0.1.0.md)
[![JDK](https://img.shields.io/badge/JDK-21-orange.svg)](https://openjdk.org/projects/jdk/21/)
[![Spring Boot](https://img.shields.io/badge/Spring%20Boot-3.2.4-green.svg)](https://spring.io/projects/spring-boot)

畅捷通 Stream Gateway 是一个高性能的 **Webhook-to-WebSocket** 透明同步桥接器。

## 🌟 核心价值
- **免公网 IP**: ISV 仅需发起 WebSocket 连接即可接收业务事件。
- **零信任架构**: 网关不持有 AppSecret，全链路签名验证代理至核心服务。
- **高性能**: 基于 Java 21 **Virtual Threads (Project Loom)** 实现，支撑海量并发连接。
- **高可用**: 支持多节点集群部署，具备毫秒级 P2P 寻址与自愈能力。

---

## 🚀 快速开始

### 1. 环境准备
- **JDK 21** (推荐 GraalVM)
- **Redis** (单机或集群)
- **Nacos MSE** (注册中心)

### 2. 构建项目
使用根目录 Makefile 一键构建：
```bash
make build-java
make build-sdk
```
构建产物位于：
- 服务端: `services/gateway-java/connector-server/target/connector-server.jar`
- SDK: `sdk/java/target/connector-sdk-java.jar`

### 3. 本地启动
```bash
java -jar services/gateway-java/connector-server/target/connector-server.jar --spring.profiles.active=localhost
```

---

## 📦 模块说明
- **`proto/`**: 跨语言 IDL 定义 (Protobuf)。
- **`services/gateway-java/`**: Java 服务端实现。
    - `connector-api`: SPI 接口契约。
    - `connector-core`: 分发、状态机、限流逻辑。
    - `connector-server`: WebSocket/HTTP 接入层。
- **`sdk/java/`**: ISV 官方 Java 接入 SDK。

---

## 🛡️ 安全与鉴权
详细算法请参考：[安全与鉴权指引](RELEASE_NOTES_v0.1.0.md#4-安全与鉴权指引-security--auth)

---

## 🛠️ 质量保证
项目遵循 **TDD (测试驱动开发)** 规范。
- **单元测试**: `mvn test`
- **全链路验收 (TCK)**: `mvn test -Dtest=TckIntegrationTest`

---
© 2026 畅捷通架构组
