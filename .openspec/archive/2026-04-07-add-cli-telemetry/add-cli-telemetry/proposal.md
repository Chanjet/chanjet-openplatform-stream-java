# Proposal: Cowen CLI 激活与活跃数据观察 (端到端方案)

## Why (背景与目标)
目前 Cowen CLI 缺乏其使用情况的可视化能力。为了建立闭环，我们需要：
1.  **CLI 端 (Producer)**：产生并异步发送遥测事件。
2.  **服务端 (Consumer)**：接收事件并持久化为结构化日志，供后期分析。

本提案现已扩展为全栈方案，确保从客户端埋点到服务端入库的完整链路。

## What Changes (主要变更)
1.  **CLI 端 (Rust)**：[已完成] 统一 UA、实现异步上报引擎、插入埋点。
2.  **服务端 (Java)**：
    *   在 `connector-server` 中增加 `/v1/telemetry/events` 接口。
    *   实现高性能异步日志追加，生成 `telemetry.log`。
    *   配置 Logback 以支持独立的遥测日志流。

## Impact (影响范围)
*   **CLI**：异步上报，零感性能损耗。
*   **Server**：新增 Ingest 接口，增加对 `/v1/telemetry/events` 的流量承载。
*   **运维**：新增 `telemetry.log` 日志文件，需配置日志清理逻辑。
