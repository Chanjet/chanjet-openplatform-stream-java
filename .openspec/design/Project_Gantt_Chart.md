# 畅捷通 Stream Gateway 项目开发甘特图 v1.0

## 1. 项目开发进度与依赖概览 (Gantt Chart)

本甘特图展示了五个并行开发流的协作节奏。核心依赖点在于 **第 1 周的契约冻结**，之后各组进入并行开发阶段。

```mermaid
gantt
    title 畅捷通 Stream Gateway 并行开发计划
    dateFormat  YYYY-MM-DD
    axisFormat  第%W周

    section A. 契约组 (Foundation)
    定义 Proto 协议与 IDL          :active, f1, 2026-03-23, 3d
    定义 Java SPI 接口 (API)      :f2, after f1, 2d
    发布 Maven BOM 与 Generated Code :milestone, f3, after f2, 0d

    section B. 领域逻辑组 (Core)
    实现消息分发逻辑 (Mock 基于 API) :c1, after f3, 5d
    实现 30min 容忍期状态机         :c2, after c1, 4d
    实现熔断与背压逻辑              :c3, after c2, 2d

    section C. 基础设施组 (Infra)
    实现 Redis 路由存储实现         :i1, after f3, 5d
    实现 Core Client (REST 代理)    :i2, after i1, 4d
    中间件集成测试 (TestContainers) :i3, after i2, 2d

    section D. 协议接入组 (Server)
    实现 WS Handler 与 Session 管理 :s1, after f3, 6d
    实现 Webhook Controller 与 P2P  :s2, after s1, 3d
    Spring Boot 装配与集成 (Wiring) :s3, after c3, 3d

    section E. SDK 与工具组 (SDK)
    Java/Python SDK 核心逻辑开发   :k1, after f1, 7d
    退避算法与心跳自愈实现          :k2, after k1, 3d
    SDK 联调测试 (基于 Mock Server) :k3, after k2, 3d

    section F. 集成与验收 (QA)
    全链路 TCK 兼容性测试          :q1, after s3, 5d
    压力测试与性能基准评估          :q2, after q1, 3d
    正式版本发布                   :milestone, q3, after q2, 0d
```

---

## 2. 关键依赖路径 (Critical Path)

### 2.1 阻塞点：契约交付 (Foundation -> All)
- **依赖说明**：所有组（Core, Infra, Server, SDK）均依赖 `proto` 定义和 `connector-api` 接口。
- **风险规避**：契约组必须在第 1 周内完成定义并发布快照版本，否则后续并行流无法启动。

### 2.2 逻辑闭环：Core & Infra -> Server
- **依赖说明**：`connector-server` 的装配逻辑依赖 `connector-core` 的业务判定以及 `connector-infra` 的具体存储实现。
- **并行方案**：Server 组在前期（s1, s2）可以先使用 `In-Memory` 的简易实现进行开发，待第 4 周再切换为真实的 `core` 和 `infra`。

### 2.3 端到端验证：SDK -> Integration
- **依赖说明**：最后的 TCK 测试需要 SDK 组交付稳定的客户端库进行联调。
- **并行方案**：SDK 组在开发期间通过 Mock Server 进行自测，不依赖后端服务的实时可用性。

---

## 3. 各阶段交付物清单

| 阶段 | 交付物 | 接收方 |
| :--- | :--- | :--- |
| **Foundation** | `proto/` 文件, `connector-api.jar` | 全体开发组 |
| **Core** | `connector-core.jar` (含单元测试) | Server 组 |
| **Infra** | `connector-infra.jar` (Redis/Core 实现) | Server 组 |
| **Server** | 可运行的网关镜像 (Docker Image) | QA 组, ISV |
| **SDK** | `sdk-java`, `sdk-python` 包 | ISV, 合作伙伴 |
| **Integration** | TCK 测试报告, 压力测试报告 | 项目组, 架构组 |

---

## 4. 进度同步机制

- **每日站会**：对齐 `proto` 是否有变更，接口是否需要微调。
- **集成周 (Week 4)**：各组将代码合并至 `main` 分支，通过 Spring DI 进行物理链路打通。
- **冒烟测试**：每完成一个子任务（如 Redis 存储实现），需在 CI 流水线中通过相应的 TCK 子集验证。
