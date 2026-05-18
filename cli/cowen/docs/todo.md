# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🔴 P0: 紧急且关键 (Critical & Urgent)
- [ ] 🎯 **消除潜在 Panic** (已规划，并入 v0.3.1 PRD 2.5): 修复 `forwarder.rs` 中 `DlqStore::new` 的 `unwrap` 调用。
    *   **实现建议**: 将 `Forwarder::new` 修改为返回 `CowenResult<Self>`，并使用 `?` 向上层传播存储初始化错误。在 `bridge.rs` 调用处增加错误捕获与日志记录，确保存储故障时能以非崩溃方式提示用户或执行降级逻辑。
- [ ] 🎯 **动态 Token 检查策略** (已规划，并入 v0.3.1 PRD 2.6): 替换 `renewer.rs` 和 `bridge.rs` 中硬编码的 10 分钟轮询间隔。
    *   **实现建议**: 
        1. **动态间隔计算**: 计算公式 `next_check = (expires_at - now) * 0.8`（或提前 15 分钟），取其与最小检查间隔（如 30s）的较大值。
        2. **引入抖动 (Jitter)**: 在计算结果上增加 `±(rand(0..60))` 秒的随机偏移，防止大量客户端在同一时刻触发刷新请求。
        3. **上限保护**: 设置最大检查间隔（如 1 小时），确保状态最终一致性。

## 🟠 P1: 高优先级 (High Priority)
- [ ] 🎯 **分拆 `cowen-common` 模块** (已规划，并入 v0.3.1 PRD 2.7): 该模块目前由于承载了配置、安全、网络及模型，已成为“上帝模块”。应将其底层工具剥离至 `cowen-infra` 或 `cowen-utils`，确保 `cowen-common` 仅包含核心模型和 SPI 契约，以减少不必要的编译依赖。
- [ ] **重构授权同步机制**: 替换 `orchestrator.rs` 中基于日志轮询的同步方式。考虑使用进程间通信 (IPC) 或更可靠的状态同步机制，解决目前严重影响首次配置成功率的不可靠问题。
- [ ] **实现优雅关机 (Graceful Shutdown)**: 显式跟踪所有异步任务（如 Token 交换、事件处理），确保守护进程退出时能安全回收资源，防止状态损坏。
- [ ] **优化 DLQ 重试逻辑**: 改进 `Forwarder::retry_message`，避免加载全量死信消息到内存。实现分页查询或按 ID 精确检索，防止大批量积压时导致的 OOM。

## 🔵 P2: 中低优先级 (Medium/Low Priority)
- [ ] **解耦进程编排逻辑**: 将 `cowen-server/src/cmd/mod.rs` 中复杂的进程监控、PID 管理和僵尸进程探测逻辑提取到独立的 `cowen-daemon` 编排组件中，提升跨平台维护性。
- [ ] **提取独立诊断模块**: 将散落在各处的 `status.rs` 和 `audit.rs` 整合为独立的 `cowen-telemetry` 模块，提升可观测性的扩展性。
- [ ] **灵活的 SSRF 防御**: 为 `forwarder.rs` 增加 Webhook 转发白名单配置，支持容器化环境（如 K8s）下的私有网段转发，而非目前硬编码的 loopback 限制。
- [ ] **构建脚本脱敏**: 移除 `Makefile` 中硬编码的 `OFFICIAL_APP_KEY`，改为从环境变量或加密 Vault 中加载，提升工程安全性。
- [ ] **拆解 Makefile**: 简化 `Makefile` 逻辑，将平台适配和容器管理逻辑拆分为独立的脚本，降低维护成本。
- [ ] **容器化测试闭环**: 消除 macOS 测试对宿主机 Homebrew 数据库的依赖，实现全量 E2E 测试的容器化一键执行，提升 CI/CD 的确定性。
- [ ] **补全 OCP 抽象**: 解决 `orchestrator.rs` 中的遗留 TODO，将系统重置 (System Reset) 逻辑彻底模块化。
