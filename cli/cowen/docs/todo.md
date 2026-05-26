# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🟢 P0: 下一版本跨平台架构重构 (Cross-Platform Architecture Refactoring)
*核心目标：解决多操作系统编译困难、AI 意外修改/破坏跨平台目标系统代码的问题。*
- [ ] **建立统一的 `sys` 目录抽象**：将涉及系统调用的底层能力下沉到统一基础 Crate (如 `cowen-infra`)。
    - [ ] 提取抽象 Trait (`sys/mod.rs`)，如 `ProcessManager`。
    - [ ] 将现有 macOS/Linux 实现迁移至 `sys/unix.rs` (`#[cfg(unix)]`)。
    - [ ] 将现有 Windows 实现迁移至 `sys/windows.rs` (`#[cfg(windows)]`)。
    - [ ] 重构业务层调用，确保仅通过 `sys::mod.rs` 的标准 Trait 交互，禁止内联 `#[cfg]`。
- [ ] **系统 API Mocking 注入**：为跨平台 Trait 接口实现依赖注入。
    - [ ] 在本地开发环境实现 `MockWindowsSys`，支持在 macOS 运行 Windows 分支的单元测试。
- [ ] **Fail Fast 编译检查左移**：更新工程构建流。
    - [ ] 在 `Makefile` 中添加 `check-cross` 靶点，执行多平台 `cargo check` (`apple-darwin`, `windows-msvc`, `linux-gnu`)。
- [ ] **确立 AI 跨平台防御规范**：更新全局规则 (`GEMINI.md`)。
    - [ ] 添加“禁止直接删除 `#[cfg(windows)]` 代码块”约束。
    - [ ] 制定新增系统底层能力时的双端占位约束 (如 `unimplemented!()`)。
    - [ ] 强制重构任务结束前主动触发 `make check-cross` 进行全平台校验。


## ️ 已归档完成事项 (Archived Completed Items)


---

## 📂 附件 (Attachments)

- 📄 **架构分析与核心源码审计报告**：包含 12 个 Crate 层级耦合度精细审计、源码精读、SOLID 原则落地以及具体解耦建议。
    - [ARCHITECTURE_AUDIT_REPORT.md](archive/ARCHITECTURE_AUDIT_REPORT.md)

# 下一版本跨平台架构重构计划 (Cross-Platform Architecture Refactoring)

## 核心目标
解决多操作系统编译困难、AI 意外修改/破坏跨平台目标系统代码的问题，确保每次发版的稳定性。

## 方案 A：同一 Crate 下基于接口拆分物理文件 (Interface-driven Physical File Isolation)

### 1. 架构调整：建立统一的 `sys` 目录
不再在业务逻辑函数中混用 `#[cfg(unix)]` 和 `#[cfg(windows)]`。将所有涉及系统调用的底层能力（如进程管理、IPC 通信、文件路径等）下沉到统一的基础 Crate（如 `cowen-infra` 或 `cowen-common`）。

**目录结构示例：**
```text
crates/cowen-infra/src/
├── lib.rs
├── sys/
│   ├── mod.rs        // 提取抽象：定义统一的 Trait (如 ProcessManager)
│   ├── unix.rs       // 承载 macOS/Linux 共有的 POSIX 兼容工具/方法 (#[cfg(unix)])
│   ├── macos.rs      // macOS 专属实现 (#[cfg(target_os = "macos")])
│   ├── linux.rs      // Linux 专属实现 (#[cfg(target_os = "linux")])
│   └── windows.rs    // Windows 专属实现 (#[cfg(windows)])
```

**代码规范约束：**
*   **依赖倒置**：业务层必须且只能通过 `sys::mod.rs` 暴露出的标准 Trait 和工厂模式进行调用。
*   **物理隔离**：所有的平台专属代码必须收敛在各自的文件中（如 `macos.rs`, `linux.rs`, `windows.rs`）。这能在物理层面上防止 AI 在修改一个平台的代码时意外破坏另一个平台的独立实现。公共 POSIX 通用逻辑收敛在 `unix.rs` 中被双端复用。

### 2. 测试策略：系统 API Mocking
*   由于遵循 **TDD Mandatory** 规则，所有 `sys` 层提供出的 Trait 接口必须利于 Mock。
*   通过依赖注入，使我们在本地 macOS 开发时可以注入一个 `MockWindowsSys`，在不切换操作系统的情况下运行所有涉及跨平台调度的核心单元测试。

### 3. 工程流改造：Fail Fast 编译检查左移
在 `Makefile` 中添加本地快速编译检查靶点，使得每次代码变更（包括 AI 生成的代码）必须强制经过跨平台语法检验，而不是拖延到发版环节。

**Makefile 增项示例：**
```makefile
.PHONY: check-cross
check-cross:
	cargo check --target aarch64-apple-darwin
	cargo check --target x86_64-pc-windows-msvc
	cargo check --target x86_64-unknown-linux-gnu
```

### 4. 设定防御性的 AI 全局规范
在 `GEMINI.md` 或相关的提示词上下文中，强制补充以下条款：
1. **禁止直接删除跨平台代码**：修改底层模块时，严禁因为“未在使用”而删除 `#[cfg(windows)]` 的专属实现代码块。
2. **保持接口对齐**：新增底层系统能力必须同时在 `unix.rs` 和 `windows.rs` 提供等价的 Trait 定义或 `unimplemented!()` 占位实现。
3. **强制验证**：凡是涉及核心底层或跨平台的重构任务完成时，**必须**主动执行 `make check-cross` 进行多目标平台的语法及类型验证。
