# cli/cowen v0.3.2 产品需求文档 (PRD)

## 1. 版本概述 (Version Overview)
v0.3.2 版本旨在从**运维自动化**、**系统稳定性**和**架构收敛**三个维度全面提升 `cowen` CLI 的工业级表现。核心目标是彻底消除用户手动编辑 YAML 文件的需求，重构脆弱的授权同步逻辑，实现生产级的优雅关机，并完成从“多进程”向“单进程”架构的实质性演进。

## 2. 核心特性 (Core Features)

### 2.1 强化 Config 命令 (Enhanced Configuration Management)
*   **需求背景**: 目前用户仍需手动修改 `app.yaml` 或 Profile YAML 来调整高级参数，这不仅门槛高且容易出错。
*   **功能描述**: 
    *   **全路径覆盖**: `cowen config set` 必须支持所有配置项，通过点分路径定位。包括但不限于：
        *   **全局配置 (Global)**: `storage.store`, `storage.db_url`, `monitor_port` 等。
        *   **Profile 配置**: `proxy_port`, `proxy_enabled`, `webhook_target`, `log.level`, `log.max_files`, `ai_enabled`, `telemetry_enabled` 等。
    *   **智能交互**:
        *   支持 `cowen config get [KEY]` 查看特定项，或 `cowen config list` 以表格/JSON 形式列出所有有效配置。
        *   设置时支持 `--global` 标志明确操作 `app.yaml`，默认为当前活跃 Profile。
    *   **校验与脱敏**:
        *   对 `port` 类字段校验范围 (1024-65535)，对 `url` 类字段校验格式。
        *   在 `get` 或 `list` 时，对敏感信息（如 `db_url` 中的密码）进行掩码处理。
*   **验收标准**:
    *   禁止用户直接通过文本编辑器修改 YAML 也能完成所有运维配置。
    *   修改 `monitor_port` 或 `proxy_port` 后，若守护进程在运行，应触发热重载或提示重启。

### 2.2 重构授权同步机制 (IPC-based Auth Sync)
*   **需求背景**: 现有的日志轮询模式在复杂文件系统环境下不可靠，导致 `init` 经常莫名超时。
*   **功能描述**: 
    *   **进程间通信 (IPC)**: CLI 与后台 Daemon 通过本地管理端口 (Monitor Port) 或 Unix Domain Socket 进行实时状态同步。
    *   **实时进度条**: `init` 过程中，后台交换令牌的每一个子步骤（获取 Code、令牌置换、写入存储）都应通过 IPC 反馈给 CLI。
    *   **详细错误透传**: 如果后台置换失败，不再只显示“超时”，而是将原始错误（如 `invalid_client`）直接透传到 CLI 终端。
*   **验收标准**:
    *   `init` 成功率显著提升，且在失败时能提供秒级的错误反馈。

### 2.3 生产级优雅关机 (Graceful Shutdown)
*   **需求背景**: 暴力关闭会导致数据库死锁残留或正在处理的消息丢失。
*   **功能描述**: 
    *   **任务追踪**: 显式跟踪所有异步任务（事件转发、Token 刷新）。
    *   **两阶段关闭**:
        1.  **停止接收**: 收到信号后立即断开 Stream 链接并停止接收新请求。
        2.  **存量清理**: 给存量任务（如正在重试的 Webhook）最多 10s 时间完成。
    *   **资源强制回收**: 超时后强制关闭所有连接池并同步刷新缓冲区到磁盘。
*   **验收标准**:
    *   在大量数据转发时执行 `stop`，不应出现数据库 `database is locked` 或消息重复推送的情况。

### 2.4 优化 DLQ 重试逻辑与性能
*   **需求背景**: 现有的全量加载模式在死信积压时会引发 OOM。
*   **功能描述**: 
    *   **分页/按需加载**: `DlqStore` 必须实现按 ID 精确查询和基于游标的分页列举。
    *   **重试限流**: `retry --all` 时应支持并发度控制，防止瞬间压垮 Webhook 目标。
*   **验收标准**:
    *   即使 DLQ 中有 10 万条消息，`retry <ID>` 操作也应在毫秒级完成，且内存占用平稳。

## 3. 架构演进：向单进程模式合并 (Single-Process Multi-Profile)

### 3.1 核心决策
本版本**正式确立单进程为默认架构**。当运行 `cowen daemon start --all` 时，系统将仅启动一个常驻进程，每个 Profile 作为该进程下的一个独立 `Task` (Worker) 运行。

### 3.2 关键设计点
*   **资源共享**: 所有 Worker 共享一个 Tokio 运行时、线程池和 `Storage` 链接池（如果是分布式存储）。
*   **隔离性保障**: 
    *   使用 `tokio::spawn` 隔离每个 Profile 的生命周期。
    *   某个 Profile Worker 的 Panic 不应导致整个进程崩溃（引入监控 Watchdog）。
*   **管理便利性**: 一个端口即可监控所有 Profile 的健康度 (Health) 和指标 (Metrics)。

## 4. 验收红线 (Acceptance Criteria)
1.  **零 YAML 手改**: 用户在完成所有高级功能配置过程中无需使用 `vim` 或 `notepad`。
2.  **单进程达成**: 多环境运行时，`ps` 进程列表中仅显示一个主进程，但各环境 Proxy 端口均能正常工作。
3.  **兼容性**: 现有 E2E 测试脚本必须适配单进程架构，且旧版 YAML 配置文件能够平滑升级。
