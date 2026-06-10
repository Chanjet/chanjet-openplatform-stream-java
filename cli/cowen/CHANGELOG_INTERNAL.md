# Cowen CLI 内部工程更新日志 (Internal Engineering Changelog)

记录底层重构、自动化构建、SDK 集成及系统自愈等非用户直观感知的工程改进。

---

## [0.4.0] - 2026-06-10

### 🏗️ 架构重构 (Architectural Refactoring)
- **Crate 分层架构重组 (Layered Architecture)**:
  - 彻底完成了工程的模块化梳理，将工程重组为 `app`、`core`、`adapters`、`services`、`plugins` 和 `tools` 等多层架构。
- **gRPC 客户端与代理层微服务能力分组 (gRPC Capability Grouping)**:
  - 重构了 gRPC Client 与 Facade，全面实施了微服务级别的能力分组设计，解耦了公共与私有能力。将全部 Capabilities 扁平化整合为安全的独立受保护区域。
- **搜索与 AI 插件深度物理隔离**:
  - 彻底将 `cowen-search-embedding` 与核心引擎解耦，并将 `cowen-ai` 降级迁移为其子 Crate，消除主引擎对体积庞大的机器学习和 ONNX Runtime 的直接依赖。

### 🔧 构建与质量管控 (Build & Quality Gate)
- **全面质量门禁扩展 (Quality Gate Expansion)**:
  - 在 CI 与 `make test` 流程中扩展了严格的代码规范检查，包含 `cargo clippy`、`cargo fmt`、跨平台交叉校验 (`cross-check`) 以及 `cargo doc` 文档健康度校验。全自动修复了历史遗留的警告，并将其设为阻塞构建的标准。
- **严格 Mock 机制强化 (E2E Mock)**:
  - 在 E2E 并发测试中进一步加强了基于本地 HTTP 拦截的严格 Mock 设计，保障了沙箱与生产环境之间的完全物理阻断。
- **无依赖工具链清理**:
  - 集成了 `cargo machete` 和 `cargo sort` 作为自动化非阻塞附加步骤，持续保持工程依赖项的精简与整洁。

---

## [0.3.6] - 2026-05-29

### 🏗️ 架构重构 (Architectural Refactoring)
- **进程树结构优化 (Process Tree Optimization)**:
  - 重构了 `cowen-sys` 中全平台的服务注册逻辑。修改了 macOS `plist`、Linux `.service` 及 Windows `sc create` 的执行入口，从原本的 `cowen daemon start --all` 变更为直接指向物理的 `cowen-daemon` 路径并传递 `--auto-start-all` 指令。
  - 移除了守护进程依赖父进程（CLI）进行配置注入的耦合，使 `cowen-daemon` 具备了完全自治的冷启动与初始化能力。
- **跨平台编译隔离加固 (Cross-Platform Compilation)**:
  - 通过条件编译 `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`，在不影响 Linux/macOS 标准输出流测试的前提下，将 Windows 版本的守护进程彻底转换为无 GUI/Console 窗口分配的纯后台执行体。

### 🔧 稳定性与可观测性加固 (Stability & Observability)
- **TCP 底层配置深度调优**:
  - 重写了 `cowen-monitor` 中的 `TcpListener` 绑定实现，直接下沉调用底层的 `socket2`，配置 `SO_REUSEADDR` 属性。在发生偶发 Crash 重启时强制抢占处在 `TIME_WAIT` 的 1588 端口，消除了因网络层限制导致的服务自愈延迟。
- **文件系统原子性改造 (Atomic IPC)**:
  - 针对 Windows 和 Unix 文件系统特性，在 `cowen-sys` 实现了严格的无锁原子文件覆盖（Write-to-Temp 然后 Rename），解决了 CLI 读取 Token 时与 Daemon 写盘操作由于无保护引发的 `Unauthorized IPC` 并发竞争漏洞。
- **运行态噪音抑制**:
  - `cowen-daemon` 的标准追踪 (`tracing`) 流重构为智能双分流管道。所有业务层级的信息与诊断日志均被拦截至 `stdout`，仅允许纯正的 `ERROR/WARN` 落地至系统服务捕获的 `stderr`。
  - 在 `telemetry.rs` 中重构了探针上报行为的容错退避策略，网络级卡顿所造成的丢包已被抑制到不可见的 `TRACE` 级别，并辅以自动放宽的三秒超时保护。
- **智能协议转换 (Intelligent Header Stripping)**:
  - 扩展了代理中间件（`cowen-server/src/daemon/proxy.rs`）的前置预检逻辑。当侦测到 HTTP 协议栈的请求方法为 `GET`/`HEAD`/`DELETE` 且请求体为空时，会在请求出站前进行底层请求头的干预并删除 `Content-Type`。此架构优化杜绝了在代理透传时上游严格校验环境（如 Tomcat 9）触发 `415 Unsupported Media Type` 的问题。

---

## [0.3.5] - 2026-05-22

### 🏗️ 架构重构 (Architectural Refactoring)
- **配置层级物理切分与加载算子 (Config Layers Isolation)**:
  - 在 `cowen-common` 中重构了 `AppConfig` 与 `Config` 模型。在 `ConfigManager` 中实现了物理隔离加载算子，引入 `validate_profile_isolation` 机制，在 Profile 层对已移出全局的键值实施硬性物理屏蔽。
  - 支持运行时通过 `COWEN_GLOBAL_*` 命名空间对全局基础设施配置进行动态覆盖覆盖，保证了配置的高优先级覆盖和运行灵活性。
- **编译参数固化注入脚手架 (Build Variable Hardening)**:
  - 交付了更加严谨的 `build.rs` 脚本。在编译阶段强行拦截 `COWEN_BUILD_*` 环境变量，如缺失直接触发 `panic!` 中断构建，彻底杜绝了源码级别的 URL 与凭证字面量硬编码。
  - 在代码调用层通过编译期 `env!()` 宏直接拉起并导出为 `const` 元数据，确保了包发布产物的 100% 确定性。
- **OCP 模块化重置 SPI 接口 (Modular SPI Reset)**:
  - 基于开闭原则 (OCP) 交付了通用的 `Resettable` Trait 抽象。核心业务 Vault、Store、Telemetry、Config 均基于此 Trait 静态实现。
  - 引入了 `inventory` 收集机制在编译期零开销自动发现注册组件，核心调度器变更为轻量级的双相调度：Phase 1 收集并输出 Dry Run 清单，Phase 2 原子物理执行清理。
- **Idempotent UDS Socket 哈希缩短**:
  - 实现了 `get_uds_path` 幂等自适应哈希算法，在发现 IPC 套接字路径越界 (SUN_LEN) 时，通过对 `app_dir` 进行 SHA-256 计算自动虚拟重映射至 `/tmp/cowen_<HASH>.sock`，增强了在各种极端深嵌套工作区下的守护进程绑定健壮性。

### 🔧 测试基础设施升级
- **E2E 失败断言全标准化转换 (Test Assertion Standardization)**:
  - 针对全量 56 个 E2E 用例的失败控制流展开深度治理。开发了高健壮性的 Python 重构转换脚本，将 152 处非标准的 inline 过程式 `exit 1` 优雅统一为标准断言 `fail_suite`，并 100% 保留了调试所需的 `cat` 打印和 `kill` 清理等上下文指令。
  - 彻底完成了测试沙箱的环境净化，高并发测试套件大回归全部 PASSED 通过，运行前后零悬挂孤儿进程残留。

---

## [0.3.3] - 2026-05-21

### 🏗️ 架构重构 (Architectural Refactoring)
- **Worker 生命周期手写状态机 (ProfileWorker State Machine)**:
  - 手写设计了 `ProfileWorker` 异步状态机，彻底剔除了对多余三方件的硬依赖，实现了对 `JoinHandle` 异步任务转移的零开销微秒级精细化管控。
  - 内置了基于指数退避时间自适应重试（最大退避至 60s）和熔断隔离逻辑（5分钟失败超 5 次则彻底熔断挂起）。熔断后必须显式调用 `restart` 重置状态机，防止了恶劣环境下的无效拉起死循环造成的系统 IO/CPU 灾难。
- **自定义 Serde JSON 寻址与重排物理坍缩 (Custom JSON Path Locator)**:
  - 在 `path_parser` 中自研了面向复杂 JSON 结构寻址的 key 匹配定位器，支持在 `unset` 删除数组元素后，索引数组物理坍缩重排。

---

## [0.3.2] - 2026-05-21

### 🔧 稳定性与性能加固
- **优雅连接秒级异步回收 (Graceful Socket Reclamation)**:
  - 重构了守护进程的多 Profile 共享调度并发架构。
  - 优雅关机信号拦截机制拦截 `SIGINT`/`SIGTERM` 时，会静默触发底层异步 Socket 的主动秒级平滑断开和生命周期销毁，确保进程强退时没有僵尸进程遗留或端口泄露发生。

---

## [0.3.1] - 2026-05-19

### 🏗️ 架构重构 (Architectural Refactoring)
- **物理 Crate 隔离实现**: 完成了全工程的物理解耦，将核心逻辑拆分为 `cowen-common`, `cowen-store`, `cowen-auth`, `cowen-search`, `cowen-server`, `cowen-infra` 等 6 个独立 Crate。通过严格的 `Cargo.toml` 依赖管理，确保了核心引擎（Server/Auth）与外围驱动（Store/Plugin）的物理层防腐。
- **插件加载基础设施 (Plugin Infrastructure)**:
    - 在 `cowen-infra` 中实现了跨平台 `PluginLoader`，基于 `libloading` 封装了 C ABI 动态链接过程。
    - 引入了“显式激活”机制，插件必须在配置文件中注册并启用才会被加载，增强了系统的确定性。
- **自适应令牌维护算法 (Adaptive Renewer)**:
    - 交付了 `calculate_next_check_delay` 核心函数，实现了 `(剩余寿命 * 0.8)` 的自适应计算逻辑。
    - 引入了 `±60s` 的随机抖动 (Jitter) 机制，并同步集成至 `renewer.rs` 与 `bridge.rs` 的循环中。

### 🔧 稳定性与性能加固
- **企业级并发死锁修复**: 
    - **SQLite 吞吐优化**: 针对 Daemon 内部多任务高并发场景，将 SQLite 连接池最大连接数从 1 放开至 5，彻底消除了异步任务在获取数据库连接时的物理排队死锁。
    - **异步 IO 锁架构**: 在 OAuth2 场景下，将同步阻塞的文件锁 `lock_exclusive()` 重构为基于 `tokio::time::sleep` 的异步轮询 `try_lock` 模式。这一变更彻底释放了在高并发换票时被占用的 Tokio 线程资源，保障了系统在高频令牌交换下的响应度。
- **存储异常防护**: 重构了 `Forwarder` 与 `DlqStore` 的初始化链路，移除所有 `unwrap()` 调用，确保在底层数据库不可达或损坏时，进程能通过 Result 链条向上反馈并实现非零退出（Exit Code 1）。

### 🤖 构建与 DevOps
- **Makefile 插件自动化**: 将 `build-plugins` 集成为全架构构建目标的预置依赖。
- **测试套件隔离性增强**: 
    - 修复了并行 E2E 测试环境下的 Shell Sourcing 逻辑，支持 40+ 脚本在原生隔离环境下的并发运行。
    - 引入了实时日志轮询探测技术，将鲁棒性测试用例的耗时从 30s+ 压缩至 **2s 以内**。

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
