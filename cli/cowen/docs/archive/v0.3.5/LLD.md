# cli/cowen v0.3.5 详细设计 (LLD)

> **版本**: v0.3.5
> **阶段**: Implementation-Ready Blueprint
> **状态**: `Archived`

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
async fn load_config(profile: &str) -> CowenResult<Config> {
    let mut global = load_app_config().await?;
    
    // 运行时环境变量覆盖 (Runtime Environment Overrides)
    // 仅限全局基础设施配置，遵循 COWEN_GLOBAL_* 命名空间
    global.apply_runtime_env_overrides("COWEN_GLOBAL_");
    
    let mut profile_cfg = load_profile_config(profile).await?;
    
    // 强隔离验证：若 Profile 中包含全局字段，需抛出校验错误并阻断启动 (Abort)
    validate_profile_isolation(&profile_cfg)?;
    
    // 逻辑合并
    profile_cfg.apply_global_defaults(global);
    Ok(profile_cfg)
}
```

### 1.3 全局配置写入算子 (Global Write Operators)
```rust
impl ConfigManager {
    pub async fn set_global_value(&self, key: &str, value: &str) -> CowenResult<()> {
        let mut app_cfg_json = self.load_app_config_as_json().await?;
        
        // 路由校验：仅允许修改全局基础设施字段
        if !is_global_infrastructure_key(key) {
            return Err(CowenError::Validation(format!("Key '{}' must be set at Profile level.", key)));
        }

        path_parser::set_by_path(&mut app_cfg_json, key, value)?;
        self.save_app_config_from_json(app_cfg_json).await
    }
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

// CLI version --debug 承接逻辑
pub fn print_version_debug() {
    println!("Build Client ID: {}", BUILTIN_CLIENT_ID);
    println!("Default Market:  {}", DEF_MARKET_URL);
}
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
pub async fn execute_reset(dry_run_mode: bool) -> CowenResult<()> {
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
```

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

### 5.1 迁移逻辑 (Migration Logic)
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

## 6. TDD 验证契约 (Testing Strategy)

### 6.1 Build Injection
*   **GIVEN**: 环境变量 `COWEN_BUILD_CLIENT_ID` 未设置。
*   **WHEN**: 执行 `cargo build`。
*   **THEN**: 编译失败并输出 `FATAL: Missing mandatory build-time variable`.

### 6.2 Config Isolation
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

## 7. LLD 执行边界

*   **命名规范**: 环境变量前缀严格遵循 `COWEN_BUILD_*` 和 `COWEN_*`。
*   **重置清理**: Vault 模块的 `reset` 必须包含彻底擦除存储介质的操作。

---

## 8. 实施可行性与 E2E 影响评估 (Feasibility & E2E Impact)

### 8.1 实施可行性 (Code Feasibility)
基于现有代码架构走查，本设计方案具有 100% 的落地可行性：
1. **配置隔离**：将 `openapi_url` 等全局字段移至 `AppConfig` 并利用 Serde 处理反序列化在现有 `cowen-common` 架构中完全可行。
2. **参数注入**：`config.rs` 中已存在 `BUILTIN_CLIENT_ID` 等硬编码 TODO 标记。引入 `build.rs` 捕获必需的 `COWEN_BUILD_*` 环境变量并替换为 `env!()` 宏调用，是标准且可行的 Rust 工程实践。
3. **OCP 模块化重置**：利用 Rust `async-trait` 结合 `inventory` 宏进行静态注册，可优雅地实现阶段性 `dry_run` 与真实的 `reset` 逻辑，消除过程化清理的弊端。

### 8.2 E2E 测试影响与变更策略 (E2E Test Impact)
由于本次升级包含 **Breaking Changes**，将对现有的 E2E 体系产生以下显著影响，并授权进行如下调整：
1. **测试脚本环境变量兼容**：为了保证向下兼容性并减少 E2E 脚本的破坏性修改，代码层面将完全保留对 `COWEN_OPENAPI_URL` 和 `COWEN_STREAM_URL` 等遗留环境变量的支持，将它们自动映射到全局配置上。这使得现有自动化脚本**无需**进行全局替换。
2. **Fixtures 环境净化**：必须从 `tests/infra/fixtures/`（如 `self-built.yaml`, `store-app.yaml`）中彻底剥离 `openapi_url` 等全局基础设施配置。这些测试所需的全局参数将统一由 `common.sh` 中的 `setup_workspace` 函数生成至孤立的 `app.yaml`。
3. **构建强制校验适配**：由于 `build.rs` 引入了对 `COWEN_BUILD_CLIENT_ID` 等变量的强依赖（缺失即 `panic!`），必须在测试构建脚手架 (如 Makefile 或 CI 脚本) 中默认注入用于测试的 Dummy 变量。
4. **Dry Run 断言增加**：需补充针对 `cowen reset --dry-run` 的回归测试用例，断言该命令不仅输出资源清单，且必须确保没有任何物理文件或数据库记录被意外清除。
