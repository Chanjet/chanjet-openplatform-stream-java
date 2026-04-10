# Cowen CLI 内部工程更新日志 (Internal Engineering Changelog)

记录底层重构、自动化构建、SDK 集成及系统自愈等非用户直观感知的工程改进。

---

## [0.1.6] - 2026-04-10

### 🛡️ 安全硬化与自愈 (Security & Resilience)
- **认证指数退避 (Auth Backoff)**: 在 `AuthClient` 中实现了基于持久化存储的指数退避逻辑。通过读取/写入 `Vault` 中的 `push_backoff_level` 和 `push_last_attempt_ts`，实现了跨进程、跨重启的请求频率限制。
- **配置系统持久化扩展**: 重构了 `Config` 结构体及 `default_with_profile` 初始化逻辑，支持 `proxy_port` 和 `proxy_enabled` 的物理持久化，彻底消除了守护进程自愈时的端口硬编码（Hardcoded Core Port）风险。
- **输出流重定向与噪音消除**: 修改了 `daemon` 和 `system` 模块的 `eprintln!` 逻辑，将非业务结果的状态类输出（如自动拉起通知、进程停止确认等）强行剥离至 `stderr`，确保 CLI 的管道友好性。

### 🤖 测试与自动化 (Testing & Automation)
- **边界值精密测试 (Boundary Testing)**: 补全了 `Auth::models` 中令牌刷新缓冲逻辑（10% 或 5min 最小边界）的单元测试，确保在时间跳秒等边界条件下认证状态的稳定性。
- **探索性测试矩阵扩展**: 将 `exploratory_ready_test.sh` 脚本从 7 步扩展为 11 步，新增了针对本地代理审计、强制环境清理及 AI 搜索索引新鲜度的专项验证步骤。

### 🏗️ 架构重构 (Architectural Changes)
- **性能优化 (Cache Matching)**: 修复了 AI 语义搜索在匹配缓存文件后缀时的逻辑错误。原本匹配 `.json` 现已修正为 `.yaml`，显著提升了接口语义索引的复用率，避免了昂贵的神经网络索引重复构建。

---

## [0.1.5] - 2026-04-10

### 🛡️ 安全硬化与自愈 (Security & Resilience)
- **编译期字符串混淆 (Compile-time Obfuscation)**: 实现了 `obfs!` 宏，在编译阶段对内置的硬编码网关 URL、API 路径及敏感的 JSON 属性键执行了异或 (XOR) 混淆。这彻底阻断了恶意攻击者通过 `strings` 等二进制逆向工具提取系统接口与凭据指纹的尝试。
- **二进制硬化与反逆向 (Binary Hardening)**: 在 release 目标中强制启用了 `strip = true`, `lto = true`，并将 `panic` 策略设置为 `"abort"`，不仅大幅缩减了体积，还有效移除了可能泄露内部工程源码路径的 Panic unwinding 异常栈信息。
- **平滑进程下线 (Graceful Shutdown)**: 在 CLI 主进程中注入了基于 `tokio::signal` 的异步信号守卫（监听 `SIGINT` 及 `SIGTERM`）。在响应系统强制关闭或用户 `Ctrl-C` 终止指令时，提供微秒级的延时缓冲，确保后台数据采集任务和缓存 IO 可以安全落盘。
- **本地凭证严格隔离**: 在加密保管库 (Vault) 刷新或写入时，严格限制文件的 Unix 权限为 `0o600`，只允许当前执行用户对安全凭据进行读写。

### 🤖 测试与自动化 (Testing & Automation)
- **TDD 脚手架与功能测试建立**: 引入 `assert_cmd` 和 `predicates` 为 CLI 设计了沙箱隔离的端到端层“功能集成测试”基座；并补全了核心工具链的边界单元测试，实现代码变更的防退化保障。
- **构建产物体检拦截**: 修改 `Makefile` 集成了 `tests/verify-binary.sh` 脚本，实现在跨平台编译输出时的产物无缝“烟雾健康验证”，杜绝坏包流入发布。

### 🏗️ 架构重构 (Architectural Changes)
- **多环境并发调度 (Multi-Profile Coordination)**: `ConfigManager` 新增动态目录扫描能力，支持解析所有实例配置。在执行 `--all` 指令时，守护进程控制器采用防抖动重置与顺序延迟调度机制，保障系统资源及端口不被瞬时耗尽。
- **抢占式下线机制 (Proactive Eviction)**: 通过点对点 HTTP P2P 接口实现了跨节点的连接驱逐逻辑。当同一客户端在节点 B 上线时，会自动通知节点 A 关闭冲突的旧连接，解决了分布式环境下的“幽灵连接”干扰。
- **Watchdog 逻辑增强**: 重构了 `ensure_daemon_running` 逻辑，集成了进程活跃度检查与自动静默拉起，提升了系统在极端环境（如系统重启）下的可用性。

---

## [0.1.4] - 2026-04-09

### 🏗️ 架构重构 (Architectural Changes)
- **SDK 运行模式调整**: 将 Rust SDK 的启动模式从异步后台任务重构为阻塞式循环（Blocking Loop），确保宿主守护进程能通过同步 Watchdog 实时监控连接的生命周期。
- **分布式清理策略**: 实现了“低频强一致 + 高频脏检查”的分层清理逻辑，确保在多节点集群环境下 `fail_start` 状态的精准重置，同时避免了高频消息对 Redis 的 IO 冲击。
### 🤖 构建与自动化 (DevOps & Build)
- **版本元数据注入**: 
  - 增强了 `build.rs` 的逻辑，实现了对当前源码 Git Hash 的实时提取。
  - 将 Git Hash 注入编译期环境变量，并集成到 CLI 的版本显示中。
- **Makefile 标准化**: 
...
  - 实现了从 `Cargo.toml` 动态提取版本号。
  - 构建产物强制遵循 `binary-v{ver}-{platform}-{arch}` 命名规范。
  - 集成 Podman 容器化构建流，支持在 macOS 下一键交叉编译 Linux amd64 满血版。
  - 自动化生成所有打包产物的 MD5 校验文件。

### 🛡️ 自愈与鲁棒性 (Resilience)
- **半连接检测**: 在 WebSocket 读取层引入了 25 秒硬超时，解决了 TCP 半打开状态下客户端无法及时重连的问题。
- **进程守卫增强**: 改进了守护进程的信号处理逻辑，确保核心任务退出时能立即触发 PID 文件清理并有序终止进程。

---

## [0.1.3] - 2026-04-07

### 🤖 异步遥测系统
- 实现了基于 `tokio::spawn` 的非阻塞事件上报引擎，确保 CLI 操作的极速响应。
- 集成了设备匿名指纹生成逻辑。
