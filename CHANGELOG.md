# 更新日志 (Changelog)

所有对畅捷通 Stream Gateway 的重要变更都将记录在本文档中。

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
