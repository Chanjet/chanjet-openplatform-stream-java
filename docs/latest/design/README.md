# 畅捷通 Stream Gateway 技术设计文档索引 (Index)

欢迎来到 **畅捷通 Stream Gateway** 技术设计文档库。本目录涵盖了从宏观架构到微服务演进、接口契约及并行开发规划的全套设计方案。

---

## 🗺️ 导航地图 (Navigation Map)

### 1. 核心架构设计 (Core Architecture)
本部分定义了系统的基石，包括整体拓扑、组件职责及领域模型。
- **[架构设计文档](./Architecture_Design.md)**: 系统的整体拓扑、核心组件职责及 P2P 转发逻辑。
- **[详细 UML 与 Package 设计](./Detailed_UML_and_Package_Design.md)**: 领域边界划分、包职责定义及核心时序图。
- **[领域模型与契约设计](./Domain_Model_and_Contract_Design.md)**: 详细定义核心类图、SPI 接口契约及 Java 21 Records 模型。

### 2. 详细技术规范 (Detailed Specifications)
定义了系统与外部及内部节点之间的交互细节。
- **[协议规范文档](./Protocol_Specification.md)**: 握手协议 (Nonce)、消息推送帧、ACK 机制及心跳规范。
- **[接口规范文档](./API_Specification.md)**: 对外（ISV）及对内（Core）的 RESTful 接口详细定义。
- **[数据模型与状态设计](./Data_Model_and_State.md)**: Redis 存储结构 (Schema) 及 30min 容忍期状态机逻辑。

### 3. 可靠性与演进 (Resilience & Evolution)
关注系统的健壮性及未来的多语言、微服务化演进方向。
- **[可靠性与容错设计](./Resilience_Design.md)**: 自愈逻辑、背压控制、熔断策略及故障切换矩阵。
- **[多语言单仓 (Monorepo) 规划](./Polyglot_Monorepo_Structure.md)**: Java, Go, Rust 混合部署的目录结构与协作契约。

### 4. 工程实践与协作 (Engineering & Collaboration)
指导如何进行多团队并行开发、模块化管理及版本交付。
- **[项目结构与 SDK 设计](./Project_Structure_and_SDK.md)**: Java 模块职责、多语言 SDK 开发原则及并行流拆解。
- **[项目开发甘特图](./Project_Gantt_Chart.md)**: 开发阶段划分、各组职责及关键依赖路径可视化。

---

## 🛠️ 技术栈速查 (Tech Stack)
- **Runtime**: JDK 21 (Virtual Threads / Project Loom)
- **Framework**: Spring Boot 4 / Spring Framework 7
- **Storage**: Redis Cluster (Routing & Nonce)
- **Contract**: Protobuf (Multi-Language)
- **Build**: Maven (Java) / Makefile (Polyglot)

---

## 📝 变更记录 (Change Log)
- **v1.1.0 (2026-03-18)**: 升级为 Polyglot Monorepo 结构，增加并行开发甘特图。
- **v1.0.0 (2026-03-18)**: 初始化全套技术方案。

---
**提示**：如需对文档进行修改，请先更新 `proto/` 契约或在对应的 SPI 接口中进行调整。
