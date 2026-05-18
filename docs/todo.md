# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🔴 P0: 紧急且关键 (Critical & Urgent)
- [ ] **消除潜在 Panic**: 修复 `forwarder.rs` 中 `DlqStore::new` 的 `unwrap` 调用。守护进程启动失败或崩溃是最高优先级修复项。
- [ ] **动态 Token 检查策略**: 替换 `renewer.rs` 和 `bridge.rs` 中硬编码的 10 分钟轮询间隔。必须实现基于 `expires_at` 的动态检查，以支持短生命周期 Token。

## 🟠 P1: 高优先级 (High Priority)
- [ ] **重构授权同步机制**: 替换 `orchestrator.rs` 中基于日志轮询的同步方式。该机制目前极不可靠，严重影响首次配置的成功率。
- [ ] **实现优雅关机 (Graceful Shutdown)**: 显式跟踪所有异步任务（如 Token 交换、事件处理），确保守护进程退出时能安全回收资源，防止状态损坏。
- [ ] **优化 DLQ 重试逻辑**: 改进 `Forwarder::retry_message`，避免加载全量死信消息到内存。防止大批量积压时导致的 OOM。
- [ ] **灵活的 SSRF 防御**: 为 `forwarder.rs` 增加 Webhook 转发白名单配置，支持容器化环境（如 K8s）下的私有网段转发。

## 🔵 P2: 中低优先级 (Medium/Low Priority)
- [ ] **构建脚本脱敏**: 移除 `Makefile` 中硬编码的 `OFFICIAL_APP_KEY`，改为从环境变量加载。提升工程安全性。
- [ ] **拆解 Makefile**: 简化 `Makefile` 逻辑，将平台适配和容器管理逻辑拆分为独立的脚本，降低维护成本。
- [ ] **容器化测试闭环**: 消除 macOS 测试对宿主机 Homebrew 数据库的依赖，提升 CI/CD 的确定性。
- [ ] **补全 OCP 抽象**: 解决 `orchestrator.rs` 中的遗留 TODO，将系统重置 (System Reset) 逻辑彻底模块化。
