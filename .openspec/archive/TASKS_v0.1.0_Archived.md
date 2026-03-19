# 畅捷通 Stream Gateway 任务清单 (Task List)

本清单用于跟踪项目的开发进度，建议后续通过 `openSpec` skills 逐步为每个任务创建详细提案。

## 🚩 阶段一：项目工程初始化 (Foundation - Scaffolding)
- [x] **Task 1.1: Monorepo 骨架搭设**
    - **内容**：创建根目录 `Makefile`，初始化 `services/` 和 `proto/` 目录。
    - **输出**：项目目录结构，根目录构建脚本。
- [x] **Task 1.2: Java 父工程与 BOM 配置**
    - **内容**：在 `services/gateway-java/` 下创建 Maven 父工程，配置 Spring Boot 4 和 JDK 21 依赖版本管理。
    - **输出**：`pom.xml`, `connector-bom` 模块。

## 📐 阶段二：契约定义 (Foundation - Contract)
- [x] **Task 2.1: Protobuf 协议定义**
    - **内容**：在 `proto/` 目录下定义 `EventFrame`, `AckFrame`, `RouteRecord` 等跨语言模型。
    - **输出**：`.proto` 文件。
- [x] **Task 2.2: Java SPI 接口契约定义**
    - **内容**：实现 `connector-api` 模块，定义 `IRouteStore`, `IConnectionManager`, `IAuthService` 等核心接口。
    - **输出**：`connector-api` 源代码。

## 🧠 阶段三：核心领域逻辑实现 (Core Domain)
- [ ] **Task 3.1: 消息分发器逻辑 (Message Dispatcher)**
    - **内容**：实现 `connector-core` 中的分发逻辑，包括本地推送与跨节点 P2P 转发判定。
    - **依赖**：Task 2.2
- [ ] **Task 3.2: 30 分钟容忍期状态机 (Tolerance Logic)**
    - **内容**：实现 AppKey 的在线/失败计时/挂起状态转换逻辑。
    - **依赖**：Task 3.1
- [ ] **Task 3.3: 背压与熔断控制 (Resilience)**
    - **内容**：实现租户级并发限流及节点级内存保护。

## 🔌 阶段四：基础设施适配 (Infra Implementation)
- [ ] **Task 4.1: Redis 路由与 Nonce 存储实现**
    - **内容**：在 `connector-infra` 中使用 Redis Cluster 实现 `IRouteStore` 和 `INonceStore`。
    - **依赖**：Task 2.2
- [ ] **Task 4.2: 畅捷通 Core REST 客户端实现**
    - **内容**：实现 `IAuthService` 和 `IPushControl`，对接 Core 侧的鉴权与推送状态 API。

## 🌐 阶段五：接入层与装配 (Server & Wiring)
- [ ] **Task 5.1: WebSocket 会话管理 (WsHandler)**
    - **内容**：在 `connector-server` 中实现连接维护、心跳及 ACK 处理。
- [ ] **Task 5.2: Webhook HTTP 接收器**
    - **内容**：实现 Webhook Dispatch 接口，接收 Core 推送并转换模型。
- [ ] **Task 5.3: 系统集成与虚拟线程配置**
    - **内容**：配置 Spring DI 将 Core/Infra 注入 Server，并启用 JDK 21 虚拟线程池。

## 📦 阶段六：SDK 开发与集成验证 (SDK & Integration)
- [ ] **Task 6.1: Java SDK 核心实现**
    - **内容**：实现基于 Java 21 HttpClient 的 ISV 接入 SDK。
- [ ] **Task 6.2: 集成测试套件 (TCK)**
    - **内容**：编写端到端测试用例，验证握手、转发、重连的全链路逻辑。

---
**提示**：每个任务开始前，请运行 `openspec-proposal-creation-cn` 针对该任务生成详细的技术实施提案。
