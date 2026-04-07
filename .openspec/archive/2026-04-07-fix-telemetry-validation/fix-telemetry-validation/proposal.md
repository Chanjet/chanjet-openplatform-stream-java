# Proposal: 强化遥测报文安全性与格式校验

## Why (背景与风险)
当前的 `TelemetryController` 直接透传 `String` 类型报文，存在以下风险：
1.  **日志注入攻击**：攻击者通过 `\n` 注入伪造的日志行。
2.  **数据污染**：非法 JSON 格式会导致后续离线分析工具链失效。
3.  **缺乏追溯性**：无法过滤掉不包含 `fingerprint` 或 `app_key` 的垃圾请求。

## What Changes (主要变更)
1.  **引入 DTO 层**：在 `connector-server` 中定义 `TelemetryEventDTO` 类。
2.  **强制反序列化校验**：使用 Spring 的 `@Valid` 与 Jackson 确保输入是合法的单一 JSON 对象。
3.  **安全再序列化**：在写入日志前，由服务端控制序列化过程，确保输出为规范的单行 JSON。

## Impact (影响范围)
*   **性能**：增加了 JSON 的一次反序列化与一次序列化开销（纳秒级影响）。
*   **可靠性**：极大提升了统计数据的真实性与系统的防御能力。
