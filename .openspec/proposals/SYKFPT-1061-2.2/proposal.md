# OpenSpec 提案：Java SPI 接口契约定义 (SYKFPT-1061-2.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-2.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 2.2: Java SPI 接口契约定义 |

---

## 1. 问题背景 (Context)
为了实现系统的高性能和跨语言演进，核心业务逻辑（Core）必须与基础设施（Redis, Core API）及传输层（WebSocket）彻底解耦。通过在 `connector-api` 模块中定义 SPI 接口，我们可以确保逻辑层能够独立于具体的中间件实现进行 TDD 开发和单元测试。

## 2. 目标 (Objectives)
- 创建 `connector-api` Maven 子模块。
- 定义核心 SPI 接口：`IRouteStore` (路由存储)、`IConnectionManager` (物理推送)、`IAuthService` (在线鉴权代理)、`IPushControl` (推送状态控制)、`INonceStore` (挑战码存储)。
- 提供清晰的 Javadoc 文档说明。

## 3. 技术设计 (Technical Design)

### 3.1 模块依赖关系
- `connector-api` 是一个纯接口模块。
- 它仅依赖 `connector-common` (用于共享的协议 Record) 和 `proto-model` (生成的 Protobuf 类)。
- 严禁依赖 Spring Boot 或具体的中间件 SDK。

### 3.2 核心接口规划
- **IRouteStore**: 负责集群内连接的物理寻址。
- **IConnectionManager**: 抽象物理 Session 交互，使 Core 逻辑无需知道 WebSocket。
- **IAuthService / IPushControl**: 封装与畅捷通 Core 后台的 REST 调用抽象。

## 4. 实施计划 (Implementation Plan)
1.  **工程搭建**: 在 `services/gateway-java/` 下创建 `connector-api` 模块并配置 `pom.xml`。
2.  **契约编写**: 按照设计文档逐个编写 Interface。
3.  **TDD 支持**: 编写一个简单的 `ContractTest` 或利用 Mockito 验证接口定义的完备性。

## 5. 验证策略 (Verification Strategy)
- **编译验证**: 运行 `mvn clean install` 确保无编译错误。
- **API 完整性**: 确保接口涵盖了 Webhook Dispatch 到 ACK 确认的全链路逻辑。

---
**审批意见**：待评审。
