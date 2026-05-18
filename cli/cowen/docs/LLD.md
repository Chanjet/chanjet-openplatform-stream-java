# cli/cowen v0.3.1 详细设计 (LLD) - 执行级蓝图

## 1. 配置热重载 (cowen-config)

### 1.1 物理模型契约
*   **边界约束**: `cowen-config` 仅依赖基础的 serde 库，绝不依赖业务逻辑层。
*   **信号**: `SIGHUP` (Value: 1)
*   **订阅者模式**: 维护并对外暴露 `tokio::sync::watch::Receiver<Config>`。

### 1.2 原子性逻辑算子
1.  **监听阶段**: `tokio::signal::unix::signal(SignalKind::hangup())` 捕获信号，或 `notify` 发现 `Write` 事件。
2.  **校验阶段 (可变性边界)**: 
    *   读取新 `app.yaml` 并尝试 `serde_yaml::from_str` 解析。
    *   **不可变字段校验**: 检查基础设施级配置（如 `app_mode`, `app_key`, `db_url`）。若发生变化，则记录 `ERROR` 日志并放弃本次更新（要求用户执行硬重启）。
    *   **可热载字段**: `log.level`, `proxy_port` (需重启内层监听器), `webhook_target` 等。
3.  **分发阶段**:
    *   通过 `tokio::sync::watch::Sender` 广播。
    *   业务层（如 `ProxyServer` Task）原子替换其局部状态。

---

## 2. 监控与健康 API (cowen-monitor)

### 2.1 物理模型契约
*   **边界约束**: `cowen-monitor` 作为全局的指标聚合器，单向被其他业务 Crate 依赖。它内部包含 Axum Web Server 逻辑。
*   **依赖倒置 (Health Probe Registry)**: 
    为避免 `cowen-monitor` 依赖具体业务模块，提供注册机制。业务模块启动时注入检查闭包：
    `pub fn register_health_probe(name: &str, probe: Box<dyn Fn() -> HealthStatus + Send + Sync>);`
*   **I/O JSON 模板 (Health Endpoint)**:
```json
{
  "status": "UP",
  "version": "0.3.1",
  "components": {
    "storage": { "status": "UP", "details": { "type": "redis", "latency": "5ms" } },
    "auth": { "status": "UP", "details": { "ticket_age": "45m" } }
  }
}
```

### 2.2 确定性逻辑：Metrics 采样
*   **Registry**: 使用 `prometheus` 的全局静态注册表。
*   **无侵入埋点**: 暴露宏给上游：
```rust
// cowen-monitor 导出的宏
cowen_monitor::counter!("cowen_proxy_requests_total", "profile" => ctx.profile, "status" => resp.status().as_str());
cowen_monitor::histogram!("cowen_proxy_request_duration_seconds", "path" => req.path());
```

---

## 3. 环境自检工具 (cowen-doctor)

### 3.1 物理模型契约
*   **边界约束**: 控制反转 (IoC)。`cowen-doctor` 只定义 Trait，不包含具体的数据库驱动或网络客户端。

```rust
// cowen-doctor/src/diagnostic.rs
pub struct DiagnosticContext {
    pub profile: String,
    pub config: Config,
}

#[async_trait]
pub trait Diagnostic: Send + Sync {
    /// 执行诊断逻辑
    /// @return Given-When-Then 风格的断言结果
    async fn check(&self, ctx: &DiagnosticContext) -> DiagnosticResult;
}

pub struct DiagnosticResult {
    pub status: HealthStatus, // OK, WARN, ERROR
    pub message: String,
    pub recommendation: Option<String>,
}
```

### 3.2 健壮性自检流
1.  调度台并行调用已注册的 `Diagnostic::check`。
2.  若某个 Check 超时（> 5s），返回强制的 `NetworkError` 并建议检查防火墙。

---

## 4. API 搜索插件化 (cowen-search / cowen-search-embedding)

### 4.1 物理模型契约
*   **边界约束**: 
    *   `cowen-search` 提供核心 Trait 和轻量级字符串匹配，作为核心依赖。
    *   `cowen-search-embedding` 是完全隔离的动态库 Crate，封装臃肿的 ONNX。

### 4.2 ABI 兼容性契约 (FFI Interface)
为确保跨动态库边界的内存安全，**严禁使用 Rust 原生集合 (`Vec`, `&str`)**，必须使用纯 C ABI 传递指针或 C 字符串：

```rust
// 跨界通信结构体 (C ABI)
#[repr(C)]
pub struct CSearchResult {
    pub ptr: *const libc::c_char, // JSON 序列化的结果数组
    pub len: usize,
}

// cowen-search/src/provider.rs
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &str;
    fn search(&self, query: &str, items: &[ApiItem], top_k: usize) -> Result<Vec<SearchResult>>;
}

// libcowen_search_embedding 导出 (在 cowen-search-embedding 中)
#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_init() -> *mut c_void {
    let provider = Box::new(EmbeddingSearchProvider::new());
    Box::into_raw(provider) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_search(
    provider_ptr: *mut c_void,
    query_ptr: *const libc::c_char,
    items_json_ptr: *const libc::c_char, // 使用 JSON 字符串传递复杂数组以规避内存布局问题
    top_k: usize,
) -> CSearchResult {
    // 内部实现：反序列化 -> 搜索 -> 序列化为 CSearchResult 返回
    // ...
}

#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_free_result(res: CSearchResult) {
    // 由插件分配的内存，必须由插件释放
    if !res.ptr.is_null() {
        let _ = std::ffi::CString::from_raw(res.ptr as *mut libc::c_char);
    }
}

#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr as *mut EmbeddingSearchProvider);
    }
}
```

### 4.3 确定性加载步骤
1.  若配置为 `embedding_search`，尝试通过 `libloading` 加载 `~/.cowen/lib/libcowen_search_embedding.[so|dylib|dll]`。
2.  获取符号 `cowen_search_provider_v1_init`。
3.  若加载失败或库不存在，退退到 `string_matching` 模式并发出警告。

---

## 5. TDD 验证契约与 E2E 验收用例

为确保新特性的稳定性，必须实现以下基于 Shell 脚本的自动化 E2E 验证用例（归档至 `tests/e2e/scripts/`）：

### 5.1 配置热重载验证 (Case: Config Hot-Reload)
*   **GIVEN**: Daemon 正在后台运行，初始代理端口为 `16001`，日志级别为 `info`。
*   **WHEN**: 动态修改 `app.yaml` 设为 `debug`，触发 `SIGHUP`。
*   **THEN**: PID 不变，后续请求输出 `DEBUG` 日志，流不断。

### 5.2 监控与健康接口验证 (Case: Metrics & Health)
*   **GIVEN**: Daemon 启动，监控端口 `9090`。
*   **WHEN**: 产生 5 次 Proxy 转发。
*   **THEN**: 访问 `http://127.0.0.1:9090/metrics`，`cowen_proxy_requests_total` 指标累加。

### 5.3 环境自检工具验证 (Case: System Doctor)
*   **GIVEN**: 错误的网络配置。
*   **WHEN**: `cowen system doctor`。
*   **THEN**: 捕获并输出 `[ERROR]`，给出具体的修复建议 (`Recommendation`)。

### 5.4 搜索插件加载与回滚验证 (Case: Search Plugin & Fallback)
*   **GIVEN**: `search_engine: embedding_search`，但缺失动态库。
*   **WHEN**: 执行 `cowen api list --search "test"`。
*   **THEN**: 成功返回结果，提示 `WARN: Advanced search plugin not found...`。

---

## 6. 变更范围约束 (Scope of Change Constraints)

为了确保 v0.3.1 的功能迭代不会对 v0.3.0 已经稳定的核心业务流程造成破坏，所有代码变更必须严格遵守以下物理隔离与依赖约束。任何越界修改将在 Code Review 或 CI 阶段被阻断。

### 6.1 新增 Crate 职责边界
1.  **`cowen-config`**: 仅允许存在与配置解析（YAML）、文件监听（notify）以及信号处理相关的逻辑。**禁止**引入任何业务模型或与具体协议（如 WebSocket, HTTP Client）相关的依赖。
2.  **`cowen-monitor`**: 仅允许存在指标采集与轻量级管理端点（Axum）。**禁止**依赖核心业务 Crate（如 `cowen-server`），只能被核心 Crate 单向依赖。
3.  **`cowen-doctor`**: 作为纯粹的 SPI 调度层，仅允许定义 `Diagnostic` Trait 并提供并发执行引擎。**禁止**在此 Crate 内部实现具体的数据库探针或网络探针，具体的探针应在调用方或专门的 Provider 中实现并注入。
4.  **`cowen-search`**: 仅包含搜索核心 Trait 及无外部依赖的字符串匹配逻辑。**禁止**在此包中引入 ONNX 或深度学习相关的依赖。
5.  **`cowen-search-embedding`**: 仅负责将模型推理逻辑打包为动态链接库 (`.so/.dylib`)。**禁止**暴露 Rust ABI 以外的接口，所有交互必须通过 C ABI 边界 (`extern "C"`) 进行。

### 6.2 现有核心的修改限制
1.  **`cowen-server` (核心引擎)**: 
    *   为了集成热重载，允许修改 `Config` 传递方式（从静态拷贝变更为 `watch::Receiver`）。
    *   为了集成监控，允许插入非阻塞的埋点宏（如 `counter!()`）。
    *   **禁止**修改原有的重连逻辑、限流退避算法以及协议解析核心逻辑。
2.  **`cowen-auth` / `cowen-store` (鉴权与存储)**: 
    *   允许这部分模块实现 `cowen-doctor` 的 `Diagnostic` Trait 以提供自身状态自检。
    *   **禁止**更改现有的 `TokenPool` 或 `Store` SPI 的核心行为契约。

### 6.3 违规行为示例 (Anti-Patterns to Avoid)
*   🚫 在 `cowen-monitor` 中硬编码读取 `cowen-auth` 的结构体以获取 Token 状态。*(正确做法：在 `cowen-auth` 中主动调用 `cowen_monitor::gauge!()` 汇报状态)*
*   🚫 在 `cowen-doctor` 中引入 `redis` crate 来检查缓存。*(正确做法：在主程序或 `cowen-store` 中实现 `Diagnostic` Trait 并注入到 Doctor)*
*   🚫 直接在 `cowen-server` 内部写 `notify` 监听逻辑。*(正确做法：由 `cowen-config` 抽象出订阅通道)*

---

## 7. DLQ 存储异常 Panic 防护 (DLQ Init Panic Protection)

### 7.1 物理模型契约 & 方法签名
*   **方法签名**:
```rust
impl Forwarder {
    /// 初始化 Forwarder 实例。
    /// * 输入: `profile: &str`, `vault: &Vault`
    /// * 输出: `Result<Self, CowenError>`
    /// * 副作用: 可能会连接 SQLite/Redis 底层存储并初始化 DlqStore 数据库连接池。
    pub fn new(profile: &str, vault: &Vault) -> Result<Self, CowenError>;
}
```

*   **健壮性异常/Action Code 矩阵**:
| 异常场景 (Exception Scenario) | 错误码 (Error Code) | 动作码 (Action Code) | 应对机制 (Handling Mechanism) |
|---|---|---|---|
| 存储连接并发占锁中 | `StorageLockConflict` | `RETRY_BACKOFF` | 记录日志并以指数级退避时间延迟尝试重连 3 次 |
| 数据库文件写保护/损坏 | `StorageCorrupted` | `FAIL_FAST` | 链式安全向上传播错误，终止进程并打印故障诊断提示 |

### 7.2 确定性逻辑算子
1.  **初始化流程 (Pseudo-code)**:
```rust
fn init_forwarder_flow(profile: &str, vault: &Vault) -> Result<Forwarder, CowenError> {
    let dlq_store = match DlqStore::new(profile, vault) {
        Ok(store) => store,
        Err(err) => {
            log::error!(target: "cowen::forwarder", "DLQ Database initialization failed: {:?}", err);
            return Err(CowenError::StorageInitFailed(err));
        }
    };
    Ok(Forwarder { dlq_store, ... })
}
```
2.  **调用层退出机制**:
    *   在 [bridge.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/cowen-server/src/cmd/bridge.rs) 守护进程启动时捕获 `Forwarder::new` 返回的错误，不再使用 `unwrap`，而是将错误打印到终端 stderr 及系统日志，之后通过 `std::process::exit(1)` 平滑安全退出。

### 7.3 TDD 验证契约
*   **GIVEN**: 一个写保护或文件锁死无法打开的 SQLite 死信队列库文件。
*   **WHEN**: 调用 `Forwarder::new` 进行初始化。
*   **THEN**: 返回 `Err(CowenError::StorageInitFailed)`，不产生 Panic 崩溃。
*   **GIVEN**: 存储初始化失败产生错误。
*   **WHEN**: `bridge` 进程启动捕获该错误。
*   **THEN**: 捕获异常，打印错误信息并安全退出（退出码 1），主进程不抛出闪退崩溃。

---

## 8. 智能动态 Token 检查与刷新策略 (Intelligent Dynamic Token Check Strategy)

### 8.1 物理模型契约 & 方法签名
*   **方法签名**:
```rust
/// 计算下一次 Token 检测和刷新的 Sleep 周期（秒数）。
/// * 输入: `expires_at: i64`, `now: i64`, `rand_jitter_offset: i64`
/// * 输出: `u64` (下一次自适应休眠延迟)
/// * 副作用: 无 (纯函数)
pub fn calculate_next_check_delay(expires_at: i64, now: i64, rand_jitter_offset: i64) -> u64;
```

*   **Token 生存期数据契约**:
```rust
pub struct TokenState {
    pub access_token: String,
    pub expires_at: i64, // 绝对生存截止 Unix 时间戳 (秒)
}
```

### 8.2 确定性逻辑算子
1.  **自适应刷新周期数学计算公式**:
    $$Interval_{raw} = (ExpiresAt - Now) \times 0.8$$
    $$Interval_{clamped} = \max(\min(Interval_{raw}, 3600), 30)$$
    $$Interval_{final} = \max(Interval_{clamped} + Jitter, 30)$$
    其中 $Jitter$ 附加抖动偏置取随机值范围为：`[-60, 60]` 秒。

2.  **算法实现逻辑 (Algorithm Pseudo-code)**:
```rust
pub fn calculate_next_check_delay(expires_at: i64, now: i64, rand_jitter_offset: i64) -> u64 {
    let remaining = expires_at - now;
    if remaining <= 0 {
        return 30; // 已过期或状态异常，执行最小保护频次
    }
    
    // 80% 寿命计算
    let raw = (remaining as f64 * 0.8) as i64;
    
    // 夹持上下限 [30s, 3600s]
    let clamped = raw.clamp(30, 3600);
    
    // 随机抖动并最终确保下限保护
    let final_interval = (clamped + rand_jitter_offset).max(30);
    final_interval as u64
}
```

### 8.3 TDD 验证契约
*   **GIVEN**: Token 剩余生存周期为 7200 秒（长寿命），抖动偏置随机计算为 +45 秒。
*   **WHEN**: 计算下一次休眠时长。
*   **THEN**: 返回 `3600 + 45 = 3645` 秒（上限截断并叠加抖动）。
*   **GIVEN**: Token 剩余生存周期为 50 秒（临近过期），抖动偏置随机计算为 -20 秒。
*   **WHEN**: 计算下一次休眠时长。
*   **THEN**: 返回 `30` 秒（满足下限边界安全夹持）。

---

## 9. 核心依赖去上帝化重构 (Decoupling & Splitting cowen-common)

### 9.1 物理层强隔离契约
*   **`cowen-common` Cargo 限制**:
    *   **Cargo.toml 声明限制**: 仅允许引入无 I/O 的高阶基础库（如 `serde`, `serde_json`, `thiserror`, `chrono`）。
    *   **禁戒依赖**: 严禁在 `cowen-common` 引入 `reqwest`、`tokio`、`redis`、`sqlx`、`libloading`。
    *   **职责边界**: 仅存放系统通用的事件模型、SPI Trait 定义、以及核心数据契约。
*   **`cowen-infra` Cargo 声明**:
    *   作为全局单向底座，存放系统环境相关的底层工具逻辑（加密/加盐 obfs、文件物理路径判定、低级时间戳工具、终端输出着色）。

### 9.2 确定性架构级拓扑依赖图
```text
           [cowen] (主程序命令行层)
              │
              ├──► [cowen-server] (Daemon 业务核心服务)
              │       │
              │       ├──► [cowen-config] (YAML 文件/信号热重载)
              │       ├──► [cowen-monitor] (Axum Metrics / 监控)
              │       └──► [cowen-doctor] (自检 Diagnostic 调度)
              │
              ├──► [cowen-search] (插件化语义搜索契约与基础匹配)
              │
              └──► [cowen-auth] (凭证轮询与刷新管理)
                      │
                      └──► [cowen-common] (稳定纯契约模型层)
                              │
                              └──► [cowen-infra] (低级基础通用工具)
```

### 9.3 TDD 验证契约 (编译时阻断测试)
*   **GIVEN**: 在 `cowen-common` 中引入含复杂底层 I/O 或高层业务模块的代码或 Crate 声明。
*   **WHEN**: 执行命令行 `cargo build --workspace` 编译。
*   **THEN**: 产生循环依赖报错或模块隔离编译报错，在编译期被拦截阻断，确保架构物理红线牢固。