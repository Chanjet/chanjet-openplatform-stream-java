# OpenSpec 提案：集成测试套件 (SYKFPT-1061-6.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-6.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 6.2: 集成测试套件 (TCK) |

---

## 1. 问题背景 (Context)
虽然各模块（Server, Core, SDK）已有各自的单元和集成测试，但缺乏一个能够模拟真实环境、打通全链路（从外部 Webhook 触发到 SDK 回调）的冒烟测试。为了确保系统各组件能够完美协同并满足性能要求，我们需要建立一套标准的 TCK。

## 2. 目标 (Objectives)
- 建立全链路集成测试场景：
    1. ISV SDK 连接网关。
    2. 外部发送 Webhook POST 到网关。
    3. 网关正确路由并推送到 SDK。
    4. SDK 处理并返回 ACK。
    5. 网关接收 ACK 并向 Webhook 发起方返回 200。
- 验证高并发下的稳定性。
- 验证在 Redis 重启或网关节点失效时的自愈能力。

## 3. 技术设计 (Technical Design)

### 3.1 测试框架选型
- **Test Runner**: JUnit 5。
- **Infrastructure**: 使用 **TestContainers** 启动 Redis 容器。
- **Mock Service**: 使用 **WireMock** 模拟畅捷通 Core 服务（用于鉴权和推送状态控制）。

### 3.2 TCK 核心用例
- **TCK-01**: 标准转发成功流 (Happy Path)。
- **TCK-02**: 无在线连接时的 30min 容忍期逻辑验证。
- **TCK-03**: 背压限流保护验证。
- **TCK-04**: 多节点 P2P 转发验证 (模拟集群环境)。

## 4. 实施计划 (Implementation Plan)
1.  **环境配置**: 在 `connector-server` 中创建一个专用的测试配置类，允许注入真实的 `connector-sdk-java`。
2.  **用例编写**: 按照 3.2 定义编写测试类。
3.  **结果审计**: 输出测试覆盖报告。

## 5. 验证策略 (Verification Strategy)
- **连通性**: 测试套件应能在本地一键运行 (`make test`)。
- **隔离性**: 测试前后应自动清理 Redis 数据和容器资源。

---
**审批意见**：待评审。
