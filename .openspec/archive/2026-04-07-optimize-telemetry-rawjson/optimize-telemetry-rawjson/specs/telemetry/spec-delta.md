# Spec Delta: RawJson Optimization

## MODIFIED Requirements

### Requirement: 遥测报文高性能清洗
SYSTEM SHALL 绕过完整的 JSON 对象解析。
FOR 接收到的原始报文,
服务端 SHALL 扫描并中和（替换或转义）所有行结束符 (`\n`, `\r`)。

#### Scenario: 自动合并单行日志
GIVEN 包含换行符的原始输入 `{"a":1}\n{"b":2}`
WHEN 写入日志时
THEN 系统 SHALL 将其记录为单行 `{"a":1} {"b":2}`
AND 确保日志流的结构不被破坏。

### Requirement: 极速合法性检查
SYSTEM SHALL 使用简单的模式匹配验证输入。
MUST 包含 `{` 开始和 `}` 结束, 且包含关键词 `"event"`。
