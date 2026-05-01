# Cowen CLI 内部工程更新日志 (Internal Engineering Changelog)

记录底层重构、自动化构建、SDK 集成及系统自愈等非用户直观感知的工程改进。

---

## [0.3.0] - 2026-04-30

### 🏗️ 架构重构 (Architectural Refactoring)
- **SPI First 架构实现**: 引入 `inventory` crate 替换原有的硬编码 `match` 分支。所有存储引擎（Store）与缓存（Cache）现在通过 `StoreBuilder` / `CacheBuilder` Trait 进行解耦，支持自动注册与自发现。
- **Vault 与 Store 解耦**: 彻底分离了凭据管理（Vault）与数据存储（Store）的职责。Vault 现在通过动态组装 `StoreBuilder` 列表来构建存储底座。
- **存储驱动扩展**: 
    - 实现了 `RedisStoreBuilder`，使 Redis 能够作为 `primary_store` 使用。
    - 统一了 SQL 驱动的连接池生命周期管理。
- **装饰器模式优化**: 重构了 `HybridStore`，使其能够作为通用的缓存装饰器包装任何实现 `Store` Trait 的后端。
- **配置默认值变更**: `StorageConfig` 默认 store 切换为 `innerdb`。
- **云原生与侧车优化 (Cloud-Native & Sidecar)**:
    - **环境变量配置优先级实现**: 重构了 `ConfigManager` 的加载逻辑。在 `Vault` 加载配置文件后，强制通过环境变量 `COWEN_*` 进行动态覆盖，确立了 `CLI参数 > 环境变量 > 配置文件` 的优先级体系。
    - **自愈式隐式初始化流程**: 在 `daemon start` 逻辑中注入了 `auto_init` 探针。当系统缺失配置文件但通过环境变量注入了 `APP_KEY` 等核心凭据时，会自动触发后台 `init::execute` 流程，并禁止递归自启动。
    - **Auth Provider SPI 增强**: 为 `AuthProvider` trait 的初始化方法补充了 `auto_start` 参数，使得 `init` 流程能精细化控制是否需要同步拉起守护进程，解决了隐式启动时的死循环风险。
    - **分布式原子同步 (Redis Lua CAS)**: 为 `RedisStore` 引入了 Lua 脚本驱动的原子 `Compare-And-Swap (CAS)` 操作，解决了高并发 Pod 启动场景下常见的“脑裂”更新与竞争写入问题。

### 🧪 测试基础设施升级
- **弹性伸缩压力测试 (Case 30/31)**: 新增了模拟 Kubernetes 场景下 4 Pod 到 8 Pod 动态扩容的 E2E 测试脚本，验证了共享 Redis 底座下的令牌一致性。
- **测试运行器稳定性增强**:
    - 为 `run_parallel.sh` 增加了对旧版本 Bash (3.2) 的兼容性处理，解决了空数组循环引发的语法错误。
    - 引入了逐案清理 (`cleanup_suite`) 与强力进程隔离 (`pkill -9`) 机制，确保并行测试环境的绝对纯净。
    - 同备更新了 PowerShell 测试运行器 (`run_suites.ps1`)，对标 Bash 版本的隔离与回收能力。

### 🛠️ 内部组件优化
- **迁移引擎 (Migrator)**: 实现了通用的跨 Store 数据搬迁逻辑，利用 Trait 抽象抹平了不同数据库间的方言差异。


## [0.2.0] - 2026-04-21

### 🤖 自动化构建与发布 (CI/CD & Release)
- **校验产物路径脱敏**: 修复了 Makefile 产生的 MD5/SHA1 校验文件中包含本地构建路径泄露的问题。
- **发布包格式优化**: 为 Windows 平台的发布产物增加了标准的 ZIP 压缩支持，并同步更新了校验逻辑。
- **安装包服务集成**: 在各平台的构建脚本中深度集成了守护进程自启动服务的自动安装逻辑（macOS postinstall, Linux install.sh, Windows setup）。
- **构建流程自动化**: 修复了 Makefile 与子模块构建约定的不一致性，确保全平台构建产物的版本号与命名规则高度统一。

### 🏗️ 架构与 SDK 优化
- **OAuth2 文档拉取优化**: 在 OAuth2 模式下，拉取 OpenAPI Spec 及接口列表时自动附加 `checkPermission=false` 参数，确保在受限授权环境下仍能正确获取 API 规约。
- **增强型故障诊断系统**:
  - **状态原因透出**: 在 `StatusEntry` 模型中引入了 `reason` 字段，实现了对 `ERROR`/`WARN` 状态的具体诱因（如 401 错误码、凭据缺失等）的直观展示。
  - **跨进程错误持久化**: 后台续约引擎（Renewer）现可将异步任务中的致命错误持久化至 Vault，解决了 CLI 前台状态与后台守护进程运行状态之间的信息断层。
  - **RefreshToken 吊销感知**: 实现了对 OAuth2 `invalid_grant` 错误的显式捕获与状态标记，使得本地“看似未过期”但实则已被服务端吊销的令牌能被准确识别为 `[REVOKED]` 状态。

### 🛡️ 安全与审计 (Security & Audit)
- **安全审计报告**: 发布了官方《Cowen CLI 安全审计报告》，系统性梳理并公开了系统的凭据管理、数据加密及日志脱敏逻辑。

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
