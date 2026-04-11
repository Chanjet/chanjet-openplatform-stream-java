# Cowen CLI 内部工程更新日志 (Internal Engineering Changelog)

记录底层重构、自动化构建、SDK 集成及系统自愈等非用户直观感知的工程改进。

---

## [0.1.5] - 2026-04-11

### 🛡️ 自愈与可靠性 (Resilience & Reliability)
- **意图驱动的生命周期管理 (Intent-Based PID Tracking)**: 重构了 `system.rs` 中的进程检查逻辑。PID 文件现在承载“运行意图”，仅在显式停止时移除。即使进程意外被系统（如 `kill -9`）杀掉，意图仍然保留，并能触发自动持久化恢复。
- **AppTicket 获取时序加固 (Ticket Handshake Race Fixes)**:
  - 在 `AuthClient` 中将 AppTicket 轮询上限从 30s 增加至 **65s**，确保其在遭遇平台 60s 频率限制（Throttling）时仍能持续轮询直至成功。
  - 维护循环（`maintenance_task`）改为动态周期：当检测到证书或 Ticket 缺失时，频率提升为 **60s** 重试；成功后恢复常规 1 小时周期。
  - 在子进程（Daemon）启动初期允许进行一次强制（`force: true`）的 push 探测逻辑，消除 CLI 触发 push 与 Daemon 就绪之间的竞态。
- **配置系统持久化扩展**: 重写了 `Config` 的初始化与解析逻辑，支持 `proxy_port` 等字段的物理持久化，彻底消除了守护进程自愈时的端口硬编码风险。
- **认证指数退避 (Auth Backoff)**: 在 `AuthClient` 中实现了基于加密保管库（Vault）持久化存储的退避算法，支持跨进程、跨重启的频率限制追踪。

### 🤖 开发与构建 (DevOps & DX)
- **启动预检机制 (Pre-flight Auth Probe)**: 在 `daemon start` 执行流中注入了前置鉴权握手，能在进程转入后台前即时拦截由于凭据配置错误导致的死循环启动。
- **构建 ID 稳定性优化 (Build ID Stabilization)**: 修正了 `build.rs` 的 ID 生成算法。在开发环境优先采用 **Git Hash** 作为 `BUILD_ID` 基准，彻底解决了通过 `cargo run` 调试时因频繁构建导致的本地版本“过期重启”循环。
- **输出流噪音治理**: 全面梳理并重定向了状态类消息输出流，将自愈通知、进程控制等交互消息剥离至 `stderr`，确保 `stdout` 输出的纯净度以支持管道化集成。

### 🏗️ 架构与 SDK 优化
- **WebSocket 握手修复**: 修复了 SDK 中 URL 拼接导致的路径参数截断问题。
- **凭据 Fail-fast 校验**: 在 SDK 运行主循环中增加了硬性空值检查，彻底杜绝了空 `app_key` 向平台发起无效鉴权的逻辑漏洞。

---

## [0.1.4] - 2026-04-09

### 🏗️ 架构重构 (Architectural Changes)
- **SDK 运行模式调整**: 将 Rust SDK 的启动模式从异步后台任务重构为阻塞式循环（Blocking Loop），确保宿主守护进程能通过同步 Watchdog 实时监控连接的生命周期。
- **分布式清理策略**: 实现了“低频强一致 + 高频脏检查”的分层清理逻辑，确保在多节点集群环境下 `fail_start` 状态的精准重置。

### 🤖 构建与自动化 (DevOps & Build)
- **版本元数据注入**: 增强了 `build.rs` 的逻辑，实现了对当前源码 Git Hash 的实时提取。
- **Makefile 标准化**: 实现了从 `Cargo.toml` 动态提取版本号，并强制遵循标准的跨平台命名规范。

---

## [0.1.3] - 2026-04-07

### 🤖 异步遥测系统
- 实现了基于 `tokio::spawn` 的非阻塞事件上报引擎。
- 集成了设备匿名指纹生成逻辑。
