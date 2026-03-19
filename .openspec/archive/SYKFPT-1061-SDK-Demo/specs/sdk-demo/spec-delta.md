# 规范差异：新增 Java SDK 演示项目

## ADDED Requirements

### Requirement: Java SDK Demo
项目 SHALL 提供官方的 `sdk/java-demo` 模块，用于演示 SDK 最佳实践。

#### Scenario: 演示核心业务消息处理
GIVEN 正确配置的 `appKey` 和 `appSecret`
WHEN 接收到 `manufactureOrderMsg`, `hsyProductMsg`, `appTicketMsg` 或 `entAuthCodeMsg`
THEN Demo 项目中的 `MessageDispatcher` 能够正确执行分发逻辑。
AND 系统日志应打印出各业务单据的关键单号或 Token 信息。

### Requirement: 业务模型 POJO 资产
Demo 项目 SHALL 提供常用的畅捷通业务模型作为参考资产。

#### Scenario: 扩展新的业务消息
ISV 可以通过参考 Demo 中的 `HsyProductMsg` 结构，快速定义并注册自定义业务消息。
