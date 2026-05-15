# cli/cowen v0.3.1 详细设计 (LLD) - 执行级蓝图

## 1. 配置热重载 (cowen-config)

### 1.1 物理模型契约
*   **边界约束**: `cowen-config` 仅依赖基础的 serde 库，绝不依赖业务逻辑层。
*   **信号**: `SIGHUP` (Value: 1)
*   **订阅者模式**: 维护并对外暴露 `tokio::sync::watch::Receiver<Config>`。

### 1.2 原子性逻辑算子
1.  **监听阶段**: `tokio::signal::unix::signal(SignalKind::hangup())` 捕获信号，或 `notify` 发现 `Write` 事件。
2.  **校验阶段**: 
    *   读取新 `app.yaml`。
    *   尝试 `serde_yaml::from_str` 解析。
    *   **语义校验**: 检查关键字段（如 `db_url`）是否发生不兼容变更。
3.  **分发阶段**:
    *   通过 `tokio::sync::watch::Sender` 广播。
    *   业务层（如 `ProxyServer` Task）原子替换其局部状态。

---

## 2. 监控与健康 API (cowen-monitor)

### 2.1 物理模型契约
*   **边界约束**: `cowen-monitor` 作为全局的指标聚合器，单向被其他业务 Crate 依赖。它内部包含 Axum Web Server 逻辑。
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
为确保动态库加载安全，定义 `extern "C"` 边界：

```rust
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