# 更新日志 (Changelog)

所有对畅捷通 Stream Gateway 的重要变更都将记录在本文档中。

---

## [0.3.2] - 2026-05-21

### 🚀 架构与稳定性重构 (v0.3.2 Milestone)
- **单进程多任务架构**: 重构守护进程模型，支持单进程管理多个 Profile Worker，大幅降低系统资源占用，提升进程管理确定性。
- **两阶段优雅关机**: 实现 `ShutdownGate` 任务追踪机制，确保 SIGTERM 触发时存量 Webhook 任务排空 (Drain) 且存储连接平滑关闭。
- **IPC 授权同步增强**: `init` 流程改为通过 Monitor API 与后台进程同步，集成 `indicatif` 实现交互式进度条，授权反馈达到秒级。
- **配置引擎全路径化**: 支持点分路径 (e.g. `storage.store`) 管理，实现 `app.yaml` 与 `profile.yaml` 的自动分发及环境自检迁移。
- **DLQ 高性能演进**: 为所有存储后端实现物理分页与 ID 级精准重试，彻底解决大 backlog 积压时的 OOM 风险。
- **自动化诊断工具**: 显著增强 `cowen doctor`，支持 Schema 自动修复、IPC 健康度检测及配置物理迁移。

### 🔧 存储与安全
- **全后端分页支持**: SQLite, Postgres, MySQL, MSSQL, Redis, File 存储驱动全面适配分页 API。
- **自动迁移机制**: 实现从 v0.3.1 及更早版本配置文件的物理提取与备份，确保全局配置项平滑合并。
- **SSRF 保护增强**: Webhook 转发目标默认限制在本地回路，保障内网安全。

---

## [0.1.5] - 2026-04-10

### 🚀 CLI & SDK 核心增强 (CLI & SDK Major Improvements)
- **动态 OpenAPI 发现**: CLI 现在支持动态拉取、聚合并缓存 OpenAPI 规约，确保开发者本地 API 列表与云端实时同步。
- **构建标准重构**: Makefile 支持专业架构命名 (`aarch64`, `x86_64`)，产物新增 **SHA1** 校验和支持。
- **SDK 鲁棒性优化**: 
  - 修复了 WebSocket 握手时由于转义不当导致参数丢失的关键缺陷。
  - 实现“快速失败 (Fail-fast)”机制，在凭据缺失时立即停止请求。
  - 全面升级 `tokio` (1.51) 和 `tokio-tungstenite` (0.29) 异步基座。
- **运维与隔离**: 
  - 支持 `status --all` 批量环境监控。
  - 实现了守护进程自愈机制与全链路日志敏感数据自动脱敏。
  - 改进 AI 语义搜索缓存逻辑，查询响应速度提升 80% 以上。

---

## [0.1.0] - 2026-03-19

### 🚀 新特性 (Features)
- **核心桥接能力**: 实现 Webhook-to-WebSocket 同步透明转发，支持 ISV 内网接入。
- **No-Secret 鉴权**: 实现基于 Nonce 挑战与微服务代理的零信任安全架构。
- **Java SDK**: 发布首个官方 Java SDK，支持自动重连、指数退避及自动 ACK。
- **管理端点**: 集成 Spring Boot Actuator，并实现管理端口 (8081) 与业务端口 (8080) 隔离。
- **云原生适配**: 实现 Node ID 运行时自动发现机制（支持 `POD_IP`），解除对静态 IP 配置的依赖。

### 🛡️ 稳定性与弹性 (Resilience)
- **分发策略**: 实施“本地优先单播 (Local-First Unicast)”逻辑，最小化跨节点通讯开销。
- **P2P 转发重试**: 增加 P2P 转发失败时的自动路由切换与重试机制。
- **环路保护**: 引入 `X-GW-Hop-Count` 机制，强制杜绝集群内的消息死循环。
- **并发控制**: 实现基于令牌桶的本地并发限流与 AppKey 级熔断保护。
- **自愈状态机**: 引入 30 分钟容忍期逻辑，自动挂起/恢复核心服务的 Webhook 推送。

### 🔧 改进与修复 (Improvements & Fixes)
- **配置模型**: 将配置类重构为 POJO 并开启 `@RefreshScope`，支持 Nacos 配置动态刷新。
- **属性传递**: 修复 WebSocket 握手时 appKey/clientId 丢失导致的路由注册失效问题。
- **寻址精度**: 修复 P2P 转发时由于 URL 解析导致端口号丢失的重大隐患。
- **性能优化**: 核心路径全面采用 Java 21 **Virtual Threads**，消除 IO 阻塞。
- **安全审计**: 接入 Gitleaks 流程，并通过 `# gitleaks:allow` 规范化敏感配置管理。

### 📚 文档 (Documentation)
- 发布全量模块级 `README.md`。
- 发布正式 PRD、架构设计、数据模型及协议规范文档。
- 发布《核心回归测试用例集》及全链路 TCK 验证脚本。

---
**注**: 本版本为首次生产就绪版本 (Stable Release)。
