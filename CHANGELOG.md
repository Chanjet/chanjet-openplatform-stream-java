# 更新日志 (Changelog)

所有对畅捷通 Stream Gateway 的重要变更都将记录在本文档中。

---

## [0.3.5] - 2026-05-22

### 🚀 全局寻址优化与构建标准化 (v0.3.5 Milestone)
- **配置分层隔离重构**: 成功将 `openapi_url`、`stream_url`、`security`、`log` 和 `search` 等公共基础设施配置移至全局 `app.yaml`。敏感业务密钥隔离在 Profile 级别，彻底消除了多环境配置文件冗余与合并冲突。
- **无感单向自动迁移**: 首次拉起新版本时，自动以当前活跃的 Profile 为准将基础设施配置上移合并至全局 `app.yaml`，并静默删除各 Profile 的冗余字段。
- **构建强校验参数注入**: 移除源码中所有硬编码内置元数据，由 `build.rs` 在编译期强制从环境变量 `COWEN_BUILD_*` 中捕获并固化为编译期常量。未指定环境变量时，直接触发编译强中断 (Abort)，确保交付包发布 100% 确定性。
- **OCP 模块化系统重置**: 定义统一的 `Resettable` Trait 并结合 `inventory` 静态注册收集器（OCP），新增任何状态化数据库、缓存或文件存储时无需改动核心调度器即可自动参与重置。同时支持 `--dry-run` 预览操作，增强了破坏性擦除的确定性。
- **IPC / UDS 路径长度自适应**: 对绝对路径超过系统 SUN_LEN 长度限制的安全套接字文件，自动使用唯一 SHA-256 哈希重映射至 `/tmp/cowen_<HASH>.sock`，对高层隐藏物理寻址差异，解决极端嵌套环境下的 IPC 异常。
- **测试失败断言全标准化**: 将 52 个 E2E 测试脚本中的 152 处非标准 inline `exit 1` 退出统一升级为规范的 `fail_suite` 标准断言 API。测试沙箱得以彻底净化，经 56 个并行回归跑测，整体以 PASSED 状态通过，架构稳定性得到强力证明。

---

## [0.3.3] - 2026-05-21

### 🚀 内部治理与交互革命 (Internal Governance Milestone)
- **ProfileWorker 状态机**: 引入确定性状态机模型（7 状态），支持**指数退避 (Backoff)** 与**熔断机制 (Circuit Breaker)**，彻底消除 `WorkerManager` 中的死锁隐患。
- **配置自治 (Identifier Locator)**: 增强配置路径解析，支持 `key:val` 寻址（如 `plugins.name:p1.path`），实现数组下标对用户的完全透明。
- **数组物理坍缩 (Collapsing)**: 实现配置项删除后的物理重排，保持索引连续且自治，支持 `+` 追加模式。
- **存储层归一化 (FileStore v3)**: 
    - 物理布局标准化为分级目录树 `vault/{profile}/{prefix}/{id}.json`，显著提升大规模数据下的 I/O 确定性。
    - 引入 `StoreItem` 泛型 Trait，消除 40% 以上的重复序列化代码。
- **自动布局迁移**: 实现 `v2_to_v3` 迁移器，系统启动时静默完成从 v0.3.2 单文件布局到 v0.3.3 目录树布局的平滑升级。

### 🔧 CLI 增强
- **配置删除指令**: 新增 `cowen config unset <PATH>` 命令。
- **可观测性升级**: `cowen status` 现在支持显示重试倒计时、重试次数及熔断原因。

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
