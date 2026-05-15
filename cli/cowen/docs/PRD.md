# cli/cowen v0.3.1 产品需求文档 (PRD)

## 1. 版本概述 (Version Overview)
v0.3.1 版本致力于增强 `cowen` 在生产环境中的 **可观测性 (Observability)**、**运维便利性 (Maintainability)** 以及 **架构灵活性 (Architectural Flexibility)**。

## 2. 核心特性 (Core Features)

### 2.1 配置热重载 (Config Hot-Reload)
*   **需求背景**: 生产环境下，修改日志级别或 Webhook 地址时不希望重启 Daemon 进程，以避免连接中断。
*   **功能描述**: 
    *   Daemon 进程支持监听 `app.yaml` 的文件变更。
    *   支持通过 `SIGHUP` 信号触发配置重新加载。
    *   重载过程应确保已建立的 WebSocket 连接和代理请求不受影响。

### 2.2 本地监控与健康 API (Local Metrics & Health API)
*   **需求背景**: 方便 K8s 存活探针检测及本地 Prometheus 指标抓取。
*   **功能描述**:
    *   在本地管理端口（默认 127.0.0.1）暴露 `/health` 接口，返回存储连通性、Daemon 存活状态。
    *   暴露 `/metrics` 接口（Prometheus 格式），统计当前连接数、请求成功率、DLQ 堆积量、Token 剩余寿命。

### 2.3 环境自检工具 (Environment Doctor)
*   **需求背景**: 快速排查因网络、权限或中间件导致的运行故障。
*   **功能描述**:
    *   新增 `cowen system doctor` 命令。
    *   自动化检查：Redis/MySQL/Postgres 连通性、开放平台网关延迟、证书有效期、本地写权限、系统限制（ulimit）等。
    *   输出格式化的诊断报告，给出修复建议。

### 2.4 API 搜索插件化 (Pluggable Search Engine)
*   **需求背景**: 目前的语义搜索依赖较重（ONNX 运行时），且搜索算法可能持续演进。希望将搜索能力抽象为插件，支持基础字符串匹配与高级语义搜索的动态切换。
*   **功能描述**: 
    *   **架构解耦**: 抽象 `SearchProvider` 接口，支持多种搜索策略。
    *   **策略切换**: 配置文件中 `search_engine` 可选值为 `string_matching` (内置) 或 `embedding_search` (动态加载)。
    *   **按需分发**: 将高级语义搜索实现为动态链接库 (如 `libcowen_search_embedding.so/dylib/dll`)。核心二进制仅包含基础匹配逻辑，减小体积。
    *   **分发优化**: 若配置为 `embedding_search` 但插件库不存在，系统应自动降级到 `string_matching` 并提供安装指引。

## 3. 技术约束 (Technical Constraints)
*   **物理隔离架构 (Physical Crate Isolation)**: 为防止代码腐化和模块越界调用，v0.3.1 的所有新特性必须封装在独立的 Cargo Crate 中（如 `cowen-config`, `cowen-monitor`, `cowen-doctor`, `cowen-search`）。核心程序通过最小化 Trait SPI 引用这些 Crate，严格禁止跨域的源码级循环依赖。
*   **稳定性**: 配置热重载严禁引起内存泄漏或进程崩溃。
*   **安全性**: 管理 API 必须严格绑定在 `127.0.0.1`，禁止外网访问。
*   **兼容性**: 插件加载机制需适配 Linux、macOS 和 Windows。

## 4. 验收标准 (Acceptance Criteria)
*   修改日志级别后，Daemon 日志输出立即生效而无需重启。
*   `curl 127.0.0.1:<port>/metrics` 能返回正确的监控数据。
*   `cowen system doctor` 能准确识别出错误的数据库配置。
*   当 `search_engine` 设为 `string_matching` 时，`api list --search` 能够快速返回关键词匹配结果。
*   当安装了高级搜索插件并设为 `embedding_search` 时，`api list --search` 支持自然语言语义理解。
