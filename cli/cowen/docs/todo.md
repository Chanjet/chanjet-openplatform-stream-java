# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)

## 🟢 P0: 下一版本跨平台架构重构 (Cross-Platform Architecture Refactoring)
*核心目标：解决多操作系统编译困难、AI 意外修改/破坏跨平台目标系统代码的问题。*
- [ ] **建立统一的 `sys` 目录抽象**：将涉及系统调用的底层能力下沉到统一基础 Crate (如 `cowen-infra`)。
    - [ ] 提取抽象 Trait (`sys/mod.rs`)，如 `ProcessManager`。
    - [ ] 将现有 macOS/Linux 实现迁移至 `sys/unix.rs` (`#[cfg(unix)]`)。
    - [ ] 将现有 Windows 实现迁移至 `sys/windows.rs` (`#[cfg(windows)]`)。
    - [ ] 重构业务层调用，确保仅通过 `sys::mod.rs` 的标准 Trait 交互，禁止内联 `#[cfg]`。
- [ ] **系统 API Mocking 注入**：为跨平台 Trait 接口实现依赖注入。
    - [ ] 在本地开发环境实现 `MockWindowsSys`，支持在 macOS 运行 Windows 分支 of 单元测试。
- [ ] **Fail Fast 编译检查左移**：更新工程构建流。
    - [ ] 在 `Makefile` 中添加 `check-cross` 靶点，执行多平台 `cargo check` (`apple-darwin`, `windows-msvc`, `linux-gnu`)。
- [ ] **确立 AI 跨平台防御规范**：更新全局规则 (`GEMINI.md`)。
    - [ ] 添加“禁止直接删除 `#[cfg(windows)]` 代码块”约束。
    - [ ] 制定新增系统底层能力时的双端占位约束 (如 `unimplemented!()`)。
    - [ ] 强制重构任务结束前主动触发 `make check-cross` 进行全平台校验。

---

## 📂 附件 (Attachments)

- 📄 **架构分析与核心源码审计报告**：包含 12 个 Crate 层级耦合度精细审计、源码精读、SOLID 原则落地以及具体解耦建议。
    - [ARCHITECTURE_AUDIT_REPORT.md](ARCHITECTURE_AUDIT_REPORT.md)
