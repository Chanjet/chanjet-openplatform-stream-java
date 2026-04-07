# Proposal: 遥测接口性能极致优化 (Raw String 方案)

## Why (优化动因)
当前的 DTO 方案涉及 `Byte -> Object -> Logic -> String` 的两次转换，在高并发场景下会产生：
1.  大量的 CPU 周期消耗（反射与解析）。
2.  频繁的内存分配与 GC 压力。

通过切换到 `Raw String` 方案，我们将转换过程简化为 `Byte -> String -> Sanitize`，在保持安全性的前提下显著提升吞吐量。

## What Changes (主要变更)
1.  **接口重构**：`TelemetryController` 不再使用 `TelemetryEventDTO`，直接接收 `@RequestBody String`。
2.  **安全清洗 (Sanitization)**：使用高效的 `String.replace` 移除所有 `\n` 和 `\r` 字符，杜绝日志注入。
3.  **轻量校验**：通过基本的字符串特征（如 `startsWith("{")` 且包含 `"event"`）进行快速合法性检查。

## Impact (影响范围)
*   **性能**：单次处理耗时降低一个数量级以上。
*   **维护性**：移除不再需要的 `TelemetryEventDTO` 类。
