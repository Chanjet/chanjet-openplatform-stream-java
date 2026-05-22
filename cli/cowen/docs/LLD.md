# cli/cowen v0.3.5 详细设计 (LLD)

> **版本**: v0.3.5
> **阶段**: Implementation-Ready Blueprint
> **状态**: `DRAFT`

## 1. 分层配置重构实现

### 1.1 物理模型契约 (Physical Model)
更新 `cowen-common` 中的 `AppConfig` 与 `Config` 结构。

```rust
// app.yaml (Global)
#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub monitor_port: u16,
    // --- 新增上移项 ---
    pub security: SecurityConfig,
    pub log: LogConfig,
    pub openapi_url: String,
    pub stream_url: String,
    pub search: SearchConfig,
}

// env.yaml (Profile)
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub app_key: String,
    pub webhook_target: String,
    pub app_mode: AuthMode,
    // 敏感业务数据保留在 Profile 级别，通常由 Vault 代理加载，但结构体中需保留以支持运行时上下文
    pub app_secret: String,
    pub encrypt_key: String,
    // 基础设施参数在 Profile 级别标记为已废弃并严禁定义
}
```

### 1.2 配置加载逻辑算子 (Logical Operators)
```rust
async fn load_config(profile: &str) -> Result<Config> {
    let global = load_app_config().await?;
    let mut profile_cfg = load_profile_config(profile).await?;
    
    // 强隔离验证：若 Profile 中包含全局字段，需抛出警告并忽略
    validate_profile_isolation(&profile_cfg)?;
    
    // 逻辑合并
    profile_cfg.apply_global_defaults(global);
    Ok(profile_cfg)
}
```

---

## 2. 构建期参数注入实现

### 2.1 build.rs 脚本逻辑
```rust
fn main() {
    let mandatory_envs = vec!["COWEN_BUILD_CLIENT_ID", "COWEN_BUILD_MARKET_URL"];
    
    for env in mandatory_envs {
        let val = std::env::var(env).unwrap_or_else(|_| {
            panic!("FATAL: Missing mandatory build-time variable: {}", env);
        });
        println!("cargo:rustc-env={}={}", env, val);
    }
}
```

### 2.2 代码引用层
```rust
pub const BUILTIN_CLIENT_ID: &str = env!("COWEN_BUILD_CLIENT_ID");
pub const DEF_MARKET_URL: &str = env!("COWEN_BUILD_MARKET_URL");
```

---
---

## 3. 系统重置 OCP 模块化实现

### 3.1 Resettable Trait 契约
```rust
#[async_trait]
pub trait Resettable: Send + Sync {
    fn name(&self) -> &str;
    /// 返回计划清理的资源列表 (文件路径或数据库表名)，承接 PRD/HLD 的 dry_run 契约
    async fn dry_run(&self) -> Vec<String>;
    /// 执行物理清理
    async fn reset(&self) -> CowenResult<()>;
}

// 采用 inventory 静态注册
inventory::collect!(Box<dyn Resettable>);
```

### 3.2 Reset 调度算子 (含 Dry Run)
```rust
pub async fn execute_reset(dry_run_mode: bool) -> Result<()> {
    for component in inventory::iter::<Box<dyn Resettable>> {
        let resources = component.dry_run().await;
        println!("Component [{}]:", component.name());
        for res in resources {
            println!("  - Plan to remove: {}", res);
        }

        if !dry_run_mode {
            component.reset().await?;
            println!("  ✅ Reset successful");
        }
    }
    Ok(())
}

---

## 4. IPC / UDS 路径健壮性逻辑

### 4.1 UDS 路径生成算子 (Idempotent Hashing)
```rust
pub fn get_uds_path() -> PathBuf {
    let app_dir = get_app_dir();
    let uds_path = app_dir.join("uds.sock");

    // SUN_LEN 限制处理 (通常 104-108 字符)
    if uds_path.to_string_lossy().len() >= 100 {
        let hash = sha256(app_dir.to_string_lossy());
        PathBuf::from(format!("/tmp/cowen_{}.sock", &hash[..16]))
    } else {
        uds_path
    }
}
```

## 5. 配置单向迁移算法


### 4.1 迁移逻辑 (Migration Logic)
1. **识别**: 检测 `app.yaml` 是否包含 v0.3.5 必需的新字段。
2. **决策**: 以 `ConfigManager::get_current_profile()` 的配置为原始样板。
3. **搬迁**:
   - 提取样板中的基础设施字段写入 `app.yaml`。
   - 遍历所有 Profile，物理删除其中的冗余字段。
4. **日志**: 输出标准化审计日志。
```text
[MIGRATION] Target: Global Configuration v0.3.5
[UP] openapi_url -> app.yaml (from profile: main)
[UP] log_level -> app.yaml
[DEL] openapi_url removed from profile: test-env
[DEL] openapi_url removed from profile: prod-env
```

---

## 5. TDD 验证契约 (Testing Strategy)

### 5.1 Build Injection
*   **GIVEN**: 环境变量 `COWEN_BUILD_CLIENT_ID` 未设置。
*   **WHEN**: 执行 `cargo build`。
*   **THEN**: 编译失败并输出 `FATAL: Missing mandatory build-time variable`.

### 5.2 Config Isolation
*   **GIVEN**: `app.yaml` 中 `openapi_url` 为 `A`。
*   **WHEN**: 尝试在 `env.yaml` 中手动写入 `openapi_url: B` 并加载。
*   **THEN**: 加载结果中 `openapi_url` 仍为 `A`，且输出隔离警告。

### 6.3 Modular Reset (Dry Run)
*   **GIVEN**: Telemetry 模块注册了 `telemetry.db`。
*   **WHEN**: 执行 `cowen reset --dry-run`。
*   **THEN**: 控制台输出包含 `telemetry.db` 的删除计划，但物理文件依然存在。

### 6.4 IPC Path Robustness
*   **GIVEN**: 用户自定义 `COWEN_HOME` 路径长度为 120 字符。
*   **WHEN**: 启动守护进程。
*   **THEN**: IPC 成功绑定至 `/tmp/cowen_<HASH>.sock`，且 CLI 可通过该路径正常通信。

### 6.5 Configuration Migration
*   **GIVEN**: 存在旧版 Profile `main` 包含 `openapi_url`。
*   **WHEN**: v0.3.5 CLI 首次运行。
*   **THEN**: `app.yaml` 成功生成并包含该 URL，同时 `main/env.yaml` 中的 `openapi_url` 字段被物理删除。


---

## 6. LLD 执行边界
*   **命名规范**: 环境变量前缀严格遵循 `COWEN_BUILD_*` 和 `COWEN_GLOBAL_*`。
*   **重置清理**: Vault 模块的 `reset` 必须包含彻底擦除存储介质的操作。
