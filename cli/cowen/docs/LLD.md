# cli/cowen v0.3.4 详细设计 (LLD)

> **版本**: v0.3.4
> **阶段**: Implementation-Ready Blueprint
> **状态**: `DRAFT`

## 1. 独立守护进程与 IPC 实现

### 1.1 物理模型契约 (IPC Protocol)
采用基于 UDS 的 JSON-RPC 简化版协议。
```rust
#[derive(Serialize, Deserialize)]
pub enum DaemonRequest {
    StartWorker { profile: String, config: Config },
    StopWorker { profile: String },
    GetStatus { profile: Option<String> },
}

#[derive(Serialize, Deserialize)]
pub enum DaemonResponse {
    Success { message: String },
    Status(HashMap<String, WorkerStatus>),
    Error { code: i32, message: String },
}
```

### 1.2 CLI 自动拉起逻辑算子
```rust
fn ensure_daemon() -> Result<UdsStream> {
    if let Ok(stream) = UdsStream::connect(UDS_PATH) {
        return Ok(stream);
    }
    let child = Command::new("cowen-daemon")
        .arg("--uds").arg(UDS_PATH)
        .spawn()?;
    // Wait for socket to appear (max 2s)
    wait_for_socket(UDS_PATH, 2000)?;
    UdsStream::connect(UDS_PATH)
}
```

---

## 2. ConfigStrategy 策略模式实现

### 2.1 SPI 契约与分发器
```rust
pub trait ConfigStrategy: Send + Sync {
    fn handle_get(&self, key: &str, current_json: &Value) -> CowenResult<Value>;
    fn handle_set(&self, key: &str, val: &str, current_json: &mut Value) -> CowenResult<()>;
}

// 示例：StorageStrategy
impl ConfigStrategy for StorageStrategy {
    fn handle_set(&self, key: &str, val: &str, root: &mut Value) -> CowenResult<()> {
        // 特有逻辑：如修改 db_url 时校验驱动合法性
        path_parser::set_by_path(root, key, val)
    }
}
```

---

## 3. 诊断持久化 (Telemetry Persistence)

### 3.1 数据库 Schema
```sql
CREATE TABLE IF NOT EXISTS telemetry_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile TEXT NOT NULL,
    event_type TEXT NOT NULL, -- 'status_change', 'error', 'backoff'
    old_status TEXT,
    new_status TEXT,
    details TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 3.2 滚动清理算法 (Retention GC)
```rust
async fn run_telemetry_gc(pool: &SqlitePool) -> Result<()> {
    // 1. 按时间清理
    sqlx::query("DELETE FROM telemetry_events WHERE created_at < date('now', '-15 days')").execute(pool).await?;
    // 2. 按条数清理（保留最新 10000 条）
    sqlx::query("DELETE FROM telemetry_events WHERE id NOT IN (SELECT id FROM telemetry_events ORDER BY id DESC LIMIT 10000)").execute(pool).await?;
    Ok(())
}
```

---

## 4. SSRF 安全等级实现

### 4.1 校验算子
```rust
pub fn validate_ssrf(url: &str, level: SecurityLevel, whitelist: &[String]) -> Result<()> {
    let host = parse_host(url)?;
    match level {
        SecurityLevel::Strict => if !is_loopback(host) { return Err(SSRFViolation); },
        SecurityLevel::Flexible => {
            if is_loopback(host) { return Ok(()); }
            if !whitelist.iter().any(|cidr| in_range(host, cidr)) { return Err(SSRFViolation); }
        },
        SecurityLevel::Disabled => {},
    }
    Ok(())
}
```

---

## 5. 诊断插件与并行调度实现

### 5.1 DiagnosticTask 契约与静态注册
```rust
#[async_trait]
pub trait DiagnosticTask: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult>;
}

// 采用 inventory 注册实现机制
inventory::collect!(Box<dyn DiagnosticTask>);
```

### 5.2 并发执行算子
```rust
async fn run_all_diagnostics(ctx: &DoctorContext) -> Vec<DiagnosticResult> {
    let mut set = tokio::task::JoinSet::new();
    for task in inventory::iter::<Box<dyn DiagnosticTask>> {
        set.spawn(task.run(ctx.clone()));
    }
    // 收集并行执行结果...
}
```

---

## 6. SQL 迁移抽象 Trait (DSL)

### 6.1 SchemaMigration 契约
```rust
#[async_trait]
pub trait SchemaMigration: Send + Sync {
    async fn get_current_version(&self, pool: &AnyPool) -> Result<u32>;
    async fn apply_sql(&self, pool: &AnyPool, sql: &str) -> Result<()>;
    fn get_migrations(&self) -> Vec<(u32, &'static str)>; // (version, sql)
}
```

---

## 7. TDD 验证契约 (Testing Strategy)

### 7.1 IPC & Startup
*   **GIVEN**: `cowen-daemon` 未运行。
*   **WHEN**: 执行 `cowen daemon status`。
*   **THEN**: CLI 成功拉起后台进程，并在 2s 后打印状态，`~/.cowen/uds.sock` 文件被创建。

### 7.2 SSRF Protection
*   **GIVEN**: 安全等级为 `Strict`。
*   **WHEN**: 设置 `webhook_target` 为 `192.168.1.1`。
*   **THEN**: 转发时立即报错 `SSRFViolation`。

### 7.3 Telemetry GC
*   **GIVEN**: 数据库中有 10,005 条记录，最旧的一条是 20 天前。
*   **WHEN**: 执行 GC。
*   **THEN**: 记录数减至 10,000，且 20 天前的记录被物理删除。

### 7.4 Doctor Plugin
*   **GIVEN**: 注册了 3 个并发诊断任务。
*   **WHEN**: 执行 `cowen doctor`。
*   **THEN**: 3 个任务并行启动，总耗时应显著小于串行执行耗时之和。
