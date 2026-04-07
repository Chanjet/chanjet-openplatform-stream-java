# Spec Delta: Telemetry Security Hardening

## MODIFIED Requirements

### Requirement: 遥测报文合法性校验
SYSTEM SHALL 校验遥测请求的内容。
FOR 接收到的 POST 请求,
服务端 SHALL 验证其为一个合法的 JSON 对象。

#### Scenario: 拒绝注入报文
GIVEN 包含换行符 `\n` 的 Payload
WHEN 发送请求到 `/v1/telemetry/events`
THEN 系统 SHALL 在日志记录时对换行符进行转义
AND 确保磁盘上的日志文件每行仅包含一个逻辑事件。

### Requirement: 字段约束规范
FOR 遥测数据记录,
SYSTEM SHALL 验证 `event` 和 `fingerprint` 字段非空。
`app_key` 字段允许为空 (Optional)。
IF `event` 或 `fingerprint` 缺失,
系统 SHALL 拒绝记录该事件。
