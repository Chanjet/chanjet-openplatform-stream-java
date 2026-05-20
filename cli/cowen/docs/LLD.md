# cli/cowen v0.3.2 详细设计 (LLD)

## 1. 增强型配置引擎 (cowen-config)

### 1.1 嵌套路径分发逻辑
`ConfigManager` 增加逻辑判定：
*   **路径前缀判断**:
    *   若路径以 `storage.` 或 `monitor_port` (全局项) 开头，则重定向操作至 `app.yaml` (AppConfig)。
    *   否则，默认操作当前 Profile 的 YAML (Config)。
*   **嵌套解析**: 利用 `serde_json::Value` 作为中间表示层，实现 `config_obj["log"]["level"] = value` 式的操作。

### 1.2 校验拦截器 (ConfigInterceptors)
在 `save` 动作前触发：
*   **PortInterceptor**: 校验端口是否在 1024-65535 之间。
*   **UrlInterceptor**: 校验 `webhook_target` 是否为合法的 http/https 格式。
*   **StorageInterceptor**: 当修改 `db_url` 时，检测协议头（sqlite/innerdb/redis）。

---

## 2. 单进程任务管理器 (WorkerManager)

### 2.1 Worker 控制块
```rust
pub struct ProfileWorker {
    profile: String,
    cancel_token: CancellationToken,
    // 用于跟踪该任务下的所有子协程
    handle_set: JoinSet<()>,
}
```

### 2.2 启动策略
1.  主 Daemon 启动。
2.  解析 `cowen daemon start --all` 命令。
3.  遍历所有有效 Profile，为每个 Profile 创建 `ProfileWorker`。
4.  在 `tokio::spawn` 中运行 `bridge::run`。
5.  **隔离性**: 包装在 `AssertUnwindSafe` 中，捕获 Panic。若某个 Worker Panic，主进程记录日志并尝试根据策略重启该 Worker，而非崩溃。

---

## 3. IPC 授权同步协议 (cowen-monitor)

### 3.1 REST 端点设计 (针对 Init 流程)
*   **`POST /v1/mgmt/auth/finalize`**:
    *   CLI 在收到浏览器 OAuth2 回调后，通过此 API 将 `session_id` 和 `code` 发送给正在运行的 Daemon。
*   **`GET /v1/mgmt/auth/progress?profile=X`**:
    *   CLI 轮询（或使用 SSE）获取令牌交换的实时进度。
    *   响应体：`{ "status": "EXCHANGING", "message": "置换令牌中...", "percent": 60 }`

---

## 4. 优雅关机控制器

### 4.1 信号处理
*   注册 `SIGTERM` / `SIGINT` 处理器。
*   触发全局 `CancellationToken`。
*   **两阶段计时器**:
    *   T+0: 各 Worker 停止监听新消息。
    *   T+10s: 若仍有任务未完成，强制执行 `Storage::shutdown()`。
