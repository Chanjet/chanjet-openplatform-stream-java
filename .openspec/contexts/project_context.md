# Project Context: Open Streaming Connector

... (已有的 project_context 内容) ...

## 核心规范: 遥测 (Telemetry)

### Requirement: 统一 User-Agent 标识
FOR 所有外发 HTTP 请求,
CLI SHALL 注入包含版本、操作系统和架构信息的 User-Agent。

#### Scenario: User-Agent 注入
GIVEN CLI 版本为 "0.1.2", 运行在 macOS arm64 环境
WHEN 执行任意涉及网络的命令
THEN 请求头应包含 `User-Agent: Cowen/0.1.2 (macos; arm64)`

### Requirement: 设备与应用双维度标识
FOR 遥测数据上报,
CLI SHALL 同时包含设备指纹和当前的 AppKey。

#### Scenario: 身份标识上报
GIVEN 用户已配置 AppKey 为 "test_key_123"
AND 设备指纹为 "fingerprint_abc"
WHEN 发送遥测事件
THEN JSON Payload 必须同时包含 `fingerprint: "fingerprint_abc"` 和 `app_key: "test_key_123"`

### Requirement: 异步静默上报
FOR 遥测上报逻辑,
SYSTEM SHALL 在后台执行且不显示任何网络相关的错误。

### Requirement: 服务端接收接口
FOR 接收来自 CLI 的遥测数据,
服务端 SHALL 暴露一个异步或极速响应的 POST 接口 `/v1/telemetry/events`。

### Requirement: 遥测日志隔离
SYSTEM SHALL 确保遥测数据不进入系统普通运行日志。
WHEN 上报数据时
THEN 该记录 SHALL 仅出现在 `telemetry.log` 中。
