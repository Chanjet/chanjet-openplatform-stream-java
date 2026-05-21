# cli/cowen v0.3.4 产品需求文档 (PRD)

> **状态**: `REVIEW`
> **版本**: v0.3.4
> **日期**: 2026-05-21

## 1. 版本背景与目标 (Context & Objectives)
在 v0.3.3 完成核心内部治理后，系统的内部架构已达到高度一致。v0.3.4 的核心目标是 **“架构解耦与工程卓越 (Architectural Decoupling & Engineering Excellence)”**。

本版本将通过将核心守护进程独立化、引入高级安全防御等级以及实现诊断数据的本地持久化，进一步提升系统的安全性、可维护性与故障回溯能力。

## 2. 核心功能需求 (Core Requirements)

### 2.1 核心剥离：独立守护进程 (Standalone cowen-daemon)
*   **需求背景**: 目前守护进程逻辑与 CLI 高度耦合。为了未来可能的后端服务化，需将其剥离。
*   **功能描述**: 
    *   将 `cowen-server` 核心编排逻辑提取为独立的二进制程序 `cowen-daemon`。
    *   **集成模式**: CLI 的 `daemon start` 命令负责**自动拉起** `cowen-daemon` 进程（如果检测到未运行），并通过 Unix Domain Socket 或 Local Loopback 建立 IPC 通道。
    *   支持独立打包与部署，脱离 CLI 运行。
*   **验收标准**: 
    *   工程目录下产生两个独立的 binary 产物。
    *   CLI 命令成功触发后台进程的冷启动及状态同步。

### 2.2 增强型安全防御：SSRF 白名单与安全等级 (Advanced SSRF Protection)
*   **需求背景**: 支持 CIDR 转发以适配内网 K8s，同时防止误配置风险。
*   **配置契约**: 在 `app.yaml` 中引入以下结构：
    ```yaml
    security:
      level: flexible  # strict, flexible, disabled
      allow_cidr: 
        - "192.168.1.0/24"
        - "10.0.0.0/8"
    ```
*   **功能描述**: 
    *   **引入安全等级 (Security Levels)**:
        *   `Strict` (默认): 仅允许 `127.0.0.1` / `localhost`。
        *   `Flexible`: 允许配置自定义 CIDR 白名单。
        *   `Disabled`: (不推荐) 不进行 SSRF 校验。
*   **验收标准**: 
    *   在 `Strict` 模式下尝试向非 loopback 地址转发必须被拦截。
    *   `Flexible` 模式下，仅允许命中 CIDR 的地址转发。

### 2.3 治理可观测性：诊断历史持久化 (Telemetry Persistence)
*   **需求背景**: 目前诊断数据仅存在于内存，无法回溯历史故障。
*   **功能描述**: 
    *   提取独立的 `cowen-telemetry` 模块。
    *   实现基于 SQLite 的历史存储，记录 Worker 状态变迁、Backoff 历史以及关键错误轨迹。
    *   **滚动清理机制 (Retention Policy)**: 建立自动 GC 逻辑。系统在启动或每 24 小时检查一次，保留**最近 15 天**或**最多 10,000 条**轨迹数据，超出部分自动物理删除。
*   **验收标准**: 
    *   即使重启进程，用户也能通过 CLI 查看过去 15 天内的历史故障轨迹。
    *   `telemetry.db` 文件大小在长时间运行后保持稳定。


### 2.4 架构深度解耦：插件化与策略模式 (Architectural Strategy)
*   **功能描述**: 
    *   **ConfigManager 策略化**: 引入 `ConfigStrategy` SPI。
    *   **Doctor 插件化**: 重构 `doctor.rs` 为基于 `DiagnosticTask` 的并发插件模型。
    *   **SQL 迁移抽象**: 提取通用的 `SchemaMigration` Trait。

### 2.5 工程脚手架升级 (Makefile Modularization)
*   **功能描述**: 将巨大的 `Makefile` 拆解为功能脚本。

## 3. 技术设计与关键技术选项确认 (Technical Design & Technology Options)

### 3.1 进程间通信 (IPC) 方案确认
*   **选型**: **Unix Domain Socket (UDS)**。
*   **依据**: UDS 相比 Local Loopback 具有更高的安全性（基于文件权限控制）和更低的延迟。
*   **兼容性**: 在 Windows 平台下自动回退至 **Named Pipes** 或 **Local Loopback**，由底层抽象库透明处理。

### 3.2 诊断数据存储选型
*   **选型**: **SQLite (via sqlx)**。
*   **依据**: 无需外部依赖，支持复杂的 SQL 查询（便于实现 15 天滚动清理算法），且与现有 `cowen-store` 的技术栈保持一致。
*   **位置**: 存储于 `~/.cowen/telemetry.db`。

### 3.3 架构解耦实现方案
*   **ConfigStrategy**: 采用 **Trait + Dynamic Dispatch (`Box<dyn ConfigStrategy>`)**。
*   **DiagnosticTask**: 采用 **inventory-style 静态注册** 或 **并发插件队列** 模式，支持异步并发执行，提升诊断响应速度。

## 4. 影响范围评估 (Impact Assessment)
*   **打包交付**: 交付物增加一个 `cowen-daemon` 二进制。
*   **存储层**: 新增 `telemetry.db` 用于记录历史轨迹。
*   **安全性**: SSRF 校验逻辑升级为基于策略的等级校验。

## 5. 任务计划 (High-level WBS)
1. **P2.1**: 剥离并构建独立的 `cowen-daemon` 二进制。
2. **P2.2**: 实现 SSRF 安全等级与 CIDR 校验。
3. **P2.3**: 提取 `cowen-telemetry` 并实现 SQLite 持久化。
4. **P2.4**: `ConfigManager` 与 `Doctor` 的策略模式重构。
5. **P2.5**: Makefile 模块化与全量回归验证。
