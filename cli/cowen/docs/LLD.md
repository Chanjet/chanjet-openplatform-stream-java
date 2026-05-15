# cli/cowen v0.3.1 详细设计 (LLD) - 执行级蓝图

## 1. 配置热重载 (Config Hot-Reload)

### 1.1 物理模型契约
*   **信号**: `SIGHUP` (Value: 1)
*   **订阅者模式**: `ConfigManager` 维护 `ArcSwap<Config>`。

### 1.2 原子性逻辑算子
1.  **监听阶段**: `tokio::signal::unix::signal(SignalKind::hangup())` 捕获信号，或 `notify` 发现 `Write` 事件。
2.  **校验阶段**: 
    *   读取新 `app.yaml`。
    *   尝试 `serde_yaml::from_str` 解析。
    *   **语义校验**: 检查关键字段（如 `db_url`）是否发生不兼容变更。
3.  **替换阶段**:
    *   调用 `ConfigManager::update(new_cfg)`。
    *   通过 `tokio::sync::watch::Sender` 发送通知。
4.  **分发阶段**:
    *   `ProxyServer` Task 接收到 `watch` 通知，原子替换内存中的 `ProxyState.config`。
    *   `Tracing` 订阅者动态调整 Log Level。

---

## 2. 监控与健康 API (Metrics & Health API)

### 2.1 I/O JSON 模板 (Health Endpoint)
*   **GET /health**
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
*   **Registry**: `cowen_common::metrics::REGISTRY` (Lazy static)。
*   **采集器注入**: 在 `ReqwestSender` 中插入中间件，统计请求耗时与状态。
```rust
// 伪代码契约
counter!("cowen_proxy_requests_total", "profile" => ctx.profile, "status" => resp.status().as_str());
histogram!("cowen_proxy_request_duration_seconds", "path" => req.path());
```

---

## 3. 环境自检工具 (System Doctor)

### 3.1 原子化方法签名
```rust
// cowen-common/src/status/diagnostic.rs

pub struct DiagnosticContext {
    pub profile: String,
    pub config: Config,
    pub vault: Arc<dyn Vault>,
}

#[async_trait]
pub trait Diagnostic: Send + Sync {
    /// 执行诊断逻辑
    /// @return Given-When-Then 风格的断言结果
    async fn check(&self, ctx: &DiagnosticContext) -> CowenResult<DiagnosticResult>;
}
```

### 3.2 健壮性自检流
1.  **Net Prober**: 
    *   执行 `GET /v1/mock/ping`。
    *   若超时 > 5s -> Action: `NetworkError` -> Reco: "检查代理或防火墙"。
2.  **Storage Prober**:
    *   执行 `SELECT 1` (SQL) 或 `PING` (Redis)。
    *   若连接拒绝 -> Action: `StorageDown` -> Reco: "检查数据库服务状态"。

---

## 4. API 搜索插件化 (Pluggable Search Engine)

### 4.1 ABI 兼容性契约 (FFI Interface)
为了确保动态库加载安全，定义 `extern "C"` 边界：

```rust
// libcowen_search_embedding 导出
#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_init() -> *mut c_void {
    let provider = Box::new(EmbeddingSearchProvider::new());
    Box::into_raw(provider) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn cowen_search_provider_v1_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr as *mut EmbeddingSearchProvider);
    }
}
```

### 4.2 确定性加载步骤
1.  **路径决策**:
    *   优先级 1: 环境变量 `COWEN_SEARCH_PLUGIN_PATH`。
    *   优先级 2: `~/.cowen/lib/libcowen_search_embedding.[so|dylib|dll]`。
2.  **安全加载**:
    *   加载动态库，校验版本符号 `cowen_plugin_abi_version`。
    *   若版本不匹配，拒绝加载并 Fallback。
3.  **生命周期**: 
    *   Plugin 指针由 `SearchManager` 持有。
    *   在 `SearchManager` 析构时调用 `v1_free`。

---

## 5. TDD 验证契约与 E2E 验收用例 (TDD & E2E Validation Suites)

为确保新特性的稳定性，必须实现以下基于 Shell 脚本的自动化 E2E 验证用例（归档至 `tests/e2e/scripts/`）：

### 5.1 配置热重载验证 (Case: Config Hot-Reload)
*   **GIVEN**: Daemon 正在后台运行，初始代理端口为 `16001`，日志级别为 `info`。
*   **WHEN**: 
    1. 动态修改 `app.yaml`，将日志级别改为 `debug`。
    2. 发送 `SIGHUP` 信号给 Daemon 进程 (或等待文件变动事件)。
*   **THEN**: 
    *   Daemon 进程 PID 不变（未发生硬重启）。
    *   随后的请求在日志中产生 `DEBUG` 级别输出。
    *   原有建立的 WebSocket 流及活动代理请求未被中断。

### 5.2 监控与健康接口验证 (Case: Metrics & Health)
*   **GIVEN**: Daemon 已启动，配置了本地管理端口（如 `9090`）。
*   **WHEN**: 通过 HTTP GET 访问 `http://127.0.0.1:9090/health`。
*   **THEN**: 响应状态码为 200，且返回格式包含 `"status": "UP"` 及存储、鉴权组件状态。
*   **WHEN**: 产生 5 次成功的 Proxy 转发请求，然后访问 `http://127.0.0.1:9090/metrics`。
*   **THEN**: 响应中 `cowen_proxy_requests_total` 指标的值增加 5。

### 5.3 环境自检工具验证 (Case: System Doctor)
*   **GIVEN**: 初始化一个存在缺陷的配置（例如 `db_url` 指向未开放的端口，或网络配置错误）。
*   **WHEN**: 执行命令 `cowen system doctor`。
*   **THEN**: 
    *   命令不应 Crash，而是正常退出。
    *   输出的报告中，正常组件标记为 `[OK]`。
    *   异常组件准确捕获，标记为 `[ERROR]`，并给出明确的 `Recommendation`（如“请检查 Redis 连接字符串及端口是否开放”）。

### 5.4 搜索插件加载与回滚验证 (Case: Search Plugin & Fallback)
*   **GIVEN**: 配置文件中设置 `search_engine: embedding_search`，但环境变量/库目录下**不存在**插件动态库。
*   **WHEN**: 执行 `cowen api list --search "test"`。
*   **THEN**: 
    *   命令成功返回匹配的 API 列表。
    *   标准错误/输出中包含警告日志："WARN: Advanced search plugin not found, falling back to string matching."
*   **GIVEN**: 放置一个实现了 C ABI 的 Mock `.so/dylib` 插件到 `lib` 目录。
*   **WHEN**: 再次执行搜索命令。
*   **THEN**: 命令成功执行，且日志表明已成功加载并调用了 `cowen_search_provider_v1_init`。
