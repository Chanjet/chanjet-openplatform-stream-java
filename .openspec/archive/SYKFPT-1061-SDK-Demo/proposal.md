# 提案：创建 Java SDK 演示项目 (sdk-java-demo)

## Why

**背景**：
- 随着 Java SDK 引入了 `MessageDispatcher` 及其自动化解密、分发能力，需要一个官方的演示项目（Demo）来展示最佳实践。
- 演示项目是 ISV 接入的重要参考资产，能够显著降低 ISV 的学习曲线和试错成本。

**当前状态**：
- 只有基础的 `sdk/java` 源码和单元测试。
- 缺乏一个模拟真实业务场景（如 T+、好生意）的完整应用示例。

**期望状态**：
- 提供一个名为 `sdk-java-demo` 的独立 Maven 项目。
- 演示如何处理四种典型的业务消息：
    1. **T+ 生产加工单新增事件** (`manufactureOrderMsg`)
    2. **好生意商品修改事件** (`hsyProductMsg`)
    3. **appTicket 消息** (`appTicketMsg`)
    4. **企业临时授权码消息** (`entAuthCodeMsg`)
- 演示如何配置 `GatewayClient` 和 `MessageDispatcher`。

## What Changes

- **新建 `sdk/java-demo` 模块**:
    - 基于 Spring Boot 构建，演示如何将 SDK 集成到 Spring 容器中。
    - 定义四种消息的业务 POJO 类。
    - 实现对应的业务逻辑处理器（Handler）。
    - 提供示例配置文件 `application.yml`。

## Impact

### 受影响的代码
- `sdk/java-demo` - 新增完整项目。

### 用户影响
- ISV 可以直接通过复制 Demo 代码来加速业务实现。
- 提供了标准的加解密与分发配置模版。

## 时间线评估
- 预计工作量：小（1-2 天）。
