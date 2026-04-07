# Spec Delta: CLI & Server Telemetry Capability

## ADDED Requirements

### Requirement: 服务端接收接口
FOR 接收来自 CLI 的遥测数据,
服务端 SHALL 暴露一个异步或极速响应的 POST 接口 `/v1/telemetry/events`。

#### Scenario: 成功接收上报
GIVEN 包含 `fingerprint` 和 `app_key` 的有效 JSON Payload
WHEN 发送 POST 请求到 `/v1/telemetry/events`
THEN 系统 SHALL 返回 `202 Accepted` 或 `200 OK`
AND 必须记录该数据到专门的 `telemetry.log` 文件。

### Requirement: 遥测日志格式规范
SYSTEM SHALL 将上报的 JSON 数据原样记录在 `telemetry.log` 中。

#### Scenario: 结构化日志持久化
WHEN 接收到 `{"event": "command_run", ...}`
THEN `telemetry.log` 文件应新增一行该 JSON 内容
AND 每行必须是一个合法的单行 JSON (JSON Line)。

### Requirement: 独立日志隔离
SYSTEM SHALL 确保遥测数据不进入系统普通运行日志。

#### Scenario: 日志流分离
WHEN 上报数据时
THEN 该记录 SHALL 仅出现在 `telemetry.log` 中
AND 不得出现在 `sys.log` 或 `audit.log` 中。
