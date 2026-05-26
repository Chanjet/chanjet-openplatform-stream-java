# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🟢 P0: 下一版本跨平台架构重构 (Cross-Platform Architecture Refactoring)
*核心目标：解决多操作系统编译困难、AI 意外修改/破坏跨平台目标系统代码的问题。*

- [ ] **建立统一的 `sys` 目录分层抽象**：将涉及系统调用的底层能力下沉到统一基础 Crate (如 `cowen-infra`)。
    - [ ] 提取平台无关抽象 Trait (`sys/mod.rs`)，定义 `ProcessManager`、`SysFingerprint` 与 `IpcBinder`。
    - [ ] 将 macOS 专属底层实现迁移至 `sys/macos.rs` (`#[cfg(target_os = "macos")]`)。
    - [ ] 将 Linux 专属底层实现迁移至 `sys/linux.rs` (`#[cfg(target_os = "linux")]`)。
    - [ ] 将公共 POSIX 兼容实现收敛至 `sys/unix.rs` (`#[cfg(unix)]`)，实现代码双端复用。
    - [ ] 将现有 Windows 专属实现迁移至 `sys/windows.rs` (`#[cfg(windows)]`)。
    - [ ] 重构外围业务层，确保所有系统调用仅面向 `sys::mod.rs` 定义的公共 Trait 编程，严禁业务代码内联平台专属 `#[cfg]` 宏。
- [ ] **系统 API Mocking 注入**：为跨平台 Trait 接口实现单元测试级依赖注入。
    - [ ] 编写符合抽象 Trait 的 `MockWindowsSys` 测试桩。
    - [ ] 支持在 macOS 本地开发环境下，一键运行 Windows 分支的全部核心单元测试。
- [ ] **Fail Fast 编译检查左移**：更新工程构建工作流。
    - [ ] 在 `Makefile` 中添加 `check-cross` 靶点，执行多平台交叉 `cargo check` (`apple-darwin`, `windows-msvc`, `linux-gnu`)。
- [ ] **确立 AI 跨平台防御规范**：更新全局规则 (`GEMINI.md`)。
    - [ ] 添加“禁止直接删除 `#[cfg(windows)]` 代码块”防御性约束。
    - [ ] 制定新增系统底层能力时的“多端接口对齐与 `unimplemented!()` 占位”规范。
    - [ ] 强制重构任务结束前主动触发 `make check-cross` 进行全目标平台语法及类型校验。

---

## 🗄️ 已归档完成事项 (Archived Completed Items)

所有历史已完成的待办事项与解耦重构任务均已物理搬迁，详细归档记录请参见：
- 📄 **[已完成任务归档记录表](archive/completed_tasks.md)** *(包含历史高危安全修复、多租户令牌自愈、cowen-doctor 插件化解耦及 Windows Service 企业级集成等里程碑成果)*

---

## 📂 附件 (Attachments)

- 📄 **[架构分析与核心源码审计报告](archive/ARCHITECTURE_AUDIT_REPORT.md)**：包含 12 个 Crate 层级耦合度精细审计、源码精读、SOLID 原则落地以及具体解耦建议。

---

# 下一版本跨平台架构重构计划书 (Cross-Platform Refactoring Design)

## 1. 方案背景与痛点
当前 `cowen` 已经高标准支持了 macOS、Linux 以及 Windows 操作系统。但因为涉及大量的系统级调用（如守护进程 Fork、SCM 服务管理、本地端口判定、机器指纹派生以及 Vault 安全加密），导致：
1. **编译依赖混乱**：部分业务逻辑代码中直接穿插了 `#[cfg(windows)]` 或 `#[cfg(unix)]`，使得可读性下降，模块边界模糊。
2. **AI 修改破坏性风险**：在单平台开发时，AI 辅助编码极易因为“当前平台未使用”而意外修改或物理剪除其他平台的代码块，造成编译灾难。
3. **本地跨平台测试断层**：由于没有对底层系统 API 进行契约隔离，开发阶段无法在本地高效模拟和覆盖其他操作系统分支的代码逻辑。

---

## 2. 方案设计：Interface-driven Physical File Isolation

为了阻断架构腐化，我们将对底层系统调用执行**契约编程（DIP 依赖倒置）**与**物理目录隔离**重构。

```mermaid
graph TD
    subgraph "业务层 (Business Layer)"
        Auth[cowen-auth] --> |仅依赖接口| SysMod[sys/mod.rs Trait]
        Server[cowen-server] --> |仅依赖接口| SysMod
    end

    subgraph "系统适配层 (sys/) - 物理隔离"
        SysMod -->|#[cfg(target_os = 'macos')]| MacOS[macos.rs]
        SysMod -->|#[cfg(target_os = 'linux')]| Linux[linux.rs]
        SysMod -->|#[cfg(windows)]| Windows[windows.rs]
        
        MacOS -->|POSIX 复用| Unix[unix.rs]
        Linux -->|POSIX 复用| Unix
        
        SysMod -.->|Unit Test Mock| Mock[MockWindowsSys]
    end
```

### 2.1 核心组件接口化设计
我们首先在 `crates/cowen-infra/src/sys/mod.rs` 中为底层三大系统行为域提取统一的、易于 Mock 的 Trait 契约：

#### A. 进程管理器契约 (`ProcessManager`)
负责统一的后台常驻、PID 探针和优雅退出机制：
```rust
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// 获取当前运行进程的物理 PID
    fn current_pid(&self) -> u32;
    /// 判定目标 PID 的进程是否在本地健康存活
    async fn is_process_alive(&self, pid: u32) -> bool;
    /// 向目标进程发送优雅停止/物理终止信号
    async fn kill_process(&self, pid: u32, force: bool) -> CowenResult<()>;
    /// 将当前进程脱离终端，平滑退化为守护进程 (Daemonize)
    async fn daemonize(&self) -> CowenResult<()>;
}
```

#### B. 硬件安全指纹契约 (`SysFingerprint`)
负责提取用于解密 Vault 密钥派生函数（KDF）的操作系统指纹：
```rust
pub trait SysFingerprint: Send + Sync {
    /// 提取操作系统级的硬件唯一指纹 (Machine ID)
    fn get_machine_id(&self) -> CowenResult<String>;
}
```

#### C. IPC 服务监听契约 (`IpcBinder`)
负责处理跨平台守护进程 IPC Socket 通信的端口绑定与强随机鉴权 Token 安全生命周期管理：
```rust
#[async_trait]
pub trait IpcBinder: Send + Sync {
    /// 动态绑定本地 TCP/UDS 监听服务，自适应防冲突分配
    async fn bind_ipc_listener(&self, addr: &str) -> CowenResult<tokio::net::TcpListener>;
    /// 读取仅对当前运行用户具有 0600 读写权限的随机鉴权 Token 字符串
    async fn load_ipc_token(&self) -> CowenResult<String>;
}
```

### 2.2 物理目录拓扑结构与条件编译
通过强编译期配置，将各平台代码物理分散在各自的文件中，绝对物理阻断 AI 的意外破坏：

```text
crates/cowen-infra/src/sys/
├── mod.rs        // 1. 契约中心与统一分发工厂
├── unix.rs       // 2. 公共 Unix 兼容 POSIX 基础函数 (#[cfg(unix)])
├── macos.rs      // 3. macOS 专属实现 (#[cfg(target_os = "macos")])
├── linux.rs      // 4. Linux 专属实现 (#[cfg(target_os = "linux")])
└── windows.rs    // 5. Windows 专属实现 (#[cfg(windows)])
```

#### `sys/mod.rs` 分发控制逻辑规范：
```rust
// 物理引入子模块
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

// 动态导出工厂装配
pub fn get_process_manager() -> Arc<dyn ProcessManager> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacProcessManager::new());
    
    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxProcessManager::new());
    
    #[cfg(windows)]
    return Arc::new(windows::WinProcessManager::new());
}

pub fn get_sys_fingerprint() -> Arc<dyn SysFingerprint> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacFingerprint::new());
    
    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxFingerprint::new());
    
    #[cfg(windows)]
    return Arc::new(windows::WinFingerprint::new());
}
```

---

## 3. 测试策略：双轨制测试与 TDD 契约
根据 **TDD Mandatory** 与 **Independent E2E Validation Standard**，为确保跨平台代码的绝对确定性，我们将测试策略设计为“双轨制”：

### 3.1 第一轨：基于 Mock / DI 的跨平台逻辑模拟（任何开发机均可运行）
本地开发阶段，我们将在 `sys` 模块的测试套件中编写包含 Given-When-Then 断言的 Mock 驱动。例如在 macOS 本地编写针对 Windows 适配层被调用时的模拟响应：
```rust
pub struct MockWindowsSys {
    pub mock_pid: u32,
    pub should_alive: bool,
}

#[async_trait]
impl ProcessManager for MockWindowsSys {
    fn current_pid(&self) -> u32 { self.mock_pid }
    async fn is_process_alive(&self, _pid: u32) -> bool { self.should_alive }
    async fn kill_process(&self, _pid: u32, _force: bool) -> CowenResult<()> { Ok(()) }
    async fn daemonize(&self) -> CowenResult<()> { Ok(()) }
}
```
这使我们无须物理切换到 Windows 环境，即可通过 `MockWindowsSys` 模拟系统调用故障、硬件指纹突变等边界场景，高标准覆盖业务调度层的所有异常分支。

### 3.2 第二轨：物理隔离的平台专属原生测试（仅在专属平台下运行）
为了验证每个平台真实系统调用的正确性（例如真实的 macOS IOKit API 是否可用，Windows 的 SCM 服务通信是否正常），我们在各平台的专属物理文件中就地编写**原生单元测试（Native Unit Tests）**。

#### A. 模块内就地测试（物理文件级自动隔离）
由于 `macos.rs` 等文件受最外层编译配置保护，我们直接在文件底部编写的测试模块将**天然且仅在专属平台下被编译和执行**：
```rust
// crates/cowen-infra/src/sys/macos.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_hardware_uuid_extraction() {
        let fingerprint = MacFingerprint::new();
        let result = fingerprint.get_machine_id();
        assert!(result.is_ok());
        let uuid = result.unwrap();
        assert_eq!(uuid.len(), 36); // 验证标准 UUID 长度
    }
}
```
当在 Linux 或 Windows 上运行 `cargo test` 时，由于编译链直接跳过了 `macos.rs` 的编译，上述 macOS 原生测试代码不会参与编译，更不会被运行。

#### B. 集成测试与特定属性守卫（多目标精细化过滤）
对于多平台共享的测试文件，可使用 `#[cfg(target_os = "...")]` 对特定的测试函数实施条件编译守卫：
```rust
#[cfg(target_os = "linux")]
#[test]
fn test_linux_etc_machine_id_fallback() {
    // 仅在真实 Linux 系统测试中被编译并执行
}
```

---

## 4. 工程流改造：Fail Fast 编译左移

通过对工程 `Makefile` 进行升级，在本地开发测试期将多平台编译检查左移（左移至本地 CI 验证阶段，严禁在 CD 阶段发版时才暴露问题）：

```makefile
# Makefile 跨平台编译语法检查强校准靶点
.PHONY: check-cross
check-cross:
	@echo "====== 开始 macOS 架构静态编译检查 ======"
	cargo check --target aarch64-apple-darwin --workspace --all-targets
	@echo "====== 开始 Linux 架构静态编译检查 ======"
	cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets
	@echo "====== 开始 Windows 架构静态编译检查 ======"
	cargo check --target x86_64-pc-windows-msvc --workspace --all-targets
	@echo "====== 所有平台跨平台编译静态检查 100% 通过 ======"
```

---

## 5. 防御性 AI 开发规范
为了保障方案在协作与 AI 开发周期中不受侵蚀，确立以下 **AI 跨平台防御安全守则 (GEMINI.md 增项)**：
1. **强行接口对齐律**：任何人在 `macos.rs` / `linux.rs` 内部新增或扩展平台专属能力时，**必须**同时在 `windows.rs` 中同步重载声明，并使用 `unimplemented!()` 占位或提供等价实现，绝对保障多平台 Crate 在任何时刻都能成功过检编译。
2. **禁止擅自物理清理**：AI 代理或开发者在单端环境下重构代码时，**绝对禁止**擅自删除其他平台的专属适配代码块。
3. **强制本地静态回归**：本地修改涉及 `sys` 目录的任何行为，在提交代码或标记任务完成前，**必须**在控制台主动发起 `make check-cross` 检验。任何未通过三端静态类型检验的代码库提交将一律做无效化拦截退回。
