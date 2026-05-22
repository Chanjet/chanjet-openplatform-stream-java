# cli/cowen v0.3.5 产品需求文档 (PRD)

> **状态**: `DRAFT`
> **版本**: v0.3.5
> **日期**: 2026-05-22

## 1. 版本背景与目标 (Context & Objectives)
在 v0.3.4 完成核心解耦与工程治理后，系统的底座已初步具备服务化能力。然而，目前配置体系中仍存在两类显著的工程痛点：
1. **基础设施配置冗余**: 部分全局性配置（如安全等级、日志、API 地址）仍绑定在 Profile 级别，导致多 Profile 环境下维护成本高。
2. **构建一致性不足**: 关键的内置默认值仍硬编码在源码中，缺乏构建时的动态注入能力，不利于私有化部署和多环境适配。

v0.3.5 的核心目标是 **“全局寻址优化与构建标准化 (Global Configuration & Build Standardization)”**。

## 2. 核心功能需求 (Core Requirements)

### 2.1 配置层级重构：核心配置迁移至应用全局 (Global Config Migration)
*   **需求背景**: 目前 `security`, `log`, `openapi_url`, `stream_url`, `search` 等配置在每个 Profile 中都存在一份拷贝，当环境地址变更时需手动修改所有 Profile。
*   **功能描述**: 
    *   **迁移路径**: 将上述基础设施配置从 `Config` (Profile 级别) 提升至 `AppConfig` (应用全局级别，对应 `app.yaml`)。
    *   **全局约束与隔离逻辑**: 
        *   Profile 级别 **严禁** 单独定义这些基础设施参数（如 `openapi_url`），配置上移后 Profile 中不得保留对应字段，从根源上杜绝合并冲突。
        *   `app_secret`、`encrypt_key` 等敏感业务数据严格属于 Profile 级别，**绝对不**存在于全局 `app.yaml` 中。
    *   **受影响模块**: `ConfigManager` 的寻址逻辑需彻底切分全局层与 Profile 层，无需进行复杂的深合并。
*   **验收标准**: 
    *   删除 Profile 配置文件中的 `openapi_url` 后，CLI 依然能通过 `app.yaml` 的全局设置正常通信。
    *   `cowen config set --global` 能够正确修改 `app.yaml` 中的对应字段。

### 2.2 构建标准化：消除硬编码默认值 (Build-time Injection)
*   **需求背景**: `BUILTIN_CLIENT_ID` 和 `DEF_MARKET_URL` 等关键值目前硬编码在 `config.rs` 中，无法通过外部流水线动态调整。
*   **功能描述**: 
    *   **参数注入**: 引入 `build.rs` 脚本，在编译时捕获环境变量（如 `COWEN_BUILD_CLIENT_ID`），并通过 `const` 或代码生成技术注入到最终二进制。
    *   **默认值清单清理**: 
        *   `BUILTIN_CLIENT_ID`
        *   `DEF_MARKET_URL`
        *   其他潜在的硬编码 URL（如预览版应用地址）。
    *   **强制校验**: 更新 Makefile，支持在构建命令中通过变量传递这些参数。**若编译时未提供必需的变量，编译过程必须报错退出 (Abort)**。
*   **验收标准**: 
    *   在不修改源码的情况下，通过 `COWEN_BUILD_CLIENT_ID=<CLIENT_ID> make build` 产生的二进制，其内置 Client ID 为 `<CLIENT_ID>`。
    *   未注入环境变量时，执行 `cargo build` 必须引发编译失败。
    *   通过 `cowen version --debug` 等指令必须能公开展示 `COWEN_BUILD_CLIENT_ID` 等注入信息以便排查问题。

### 2.3 补全 OCP 抽象：系统重置模块化 (Modular System Reset)
*   **需求背景**: 目前 `cowen reset` 逻辑高度过程化，散落在各命令实现中。为了符合开闭原则（OCP），新增存储或组件时需要能自动参与重置流程。
*   **功能描述**: 
    *   **抽象 Trait**: 定义 `Resettable` Trait。
    *   **自动注册**: 各核心组件（Vault, Store, Telemetry, Config）实现该 Trait 并通过插件化方式注册。
    *   **调度重构**: 重构 `cmd/reset.rs`，使其仅作为调度器遍历并执行所有已注册组件的重置方法。
    *   **能力边界**: `Resettable` 必须支持彻底清理数据库 (Database)。
    *   **Dry Run 模式**: 必须支持 `--dry-run` 参数，在实际执行重置前，预览将要删除的文件路径和数据库记录。
*   **验收标准**: 
    *   运行 `cowen reset --dry-run` 仅输出计划删除的资源清单，不产生任何物理删除或副作用。
    *   执行正式重置后，通过该机制注册的数据库表和缓存文件被彻底清空。

## 3. 技术设计与关键技术选项确认 (Technical Design & Technology Options)

### 3.1 跨平台 IPC 路径健壮性
*   **UDS 路径长度限制 (SUN_LEN)**: 虽然在 v0.3.4 中已解决部分 UDS 路径超长问题，但在 v0.3.5 调整全局 `app_dir` 解析逻辑时，**必须**保证现有 Hash 缩短算法的健壮性，确保任何自定义工作区下均不会触发 IPC 绑定失败。

### 3.2 构建注入实现
*   **方案**: 使用 `build.rs` 捕获必需的 `env`，并通过 `cargo:rustc-env=KEY=VALUE` 输出。
*   **代码调用**: 使用 `env!("KEY")` 宏获取编译期变量（该宏在找不到变量时会在编译期报错，完美契合“必须报错退出”的需求）。

## 4. 影响范围评估 (Impact Assessment)
*   **向下兼容性 (配置迁移)**: 需提供配置自动迁移逻辑。在 v0.3.5 首次运行时进行平滑搬迁：
    *   **冲突解决**: 当多 Profile 配置不一致时，严格以 **当前激活的 Profile (Current Profile)** 的配置为准合并至全局。
    *   **断代更新**: 本版本开始不再对之前版本的配置机制提供历史兼容，迁移为单向操作，无需实现自动回滚。
*   **开发体验**: 减少了在 `init` 多个 Profile 时输入重复参数的需要。

## 5. 任务计划 (High-level WBS)
1. **P1.1**: 重构 `AppConfig` 结构并更新 `app.yaml` 读写逻辑，实现 Profile 与 Global 的彻底隔离。
2. **P1.2**: 实现 `build.rs` 强校验注入机制，清理 `config.rs` 硬编码项。
3. **P1.3**: 系统重置逻辑 OCP 模块化重构 (包含 Dry Run)。
4. **P1.4**: 编写配置单向迁移脚本 (以 Current Profile 为准)。
5. **P1.5**: 全量回归验证及 IPC 路径健壮性测试。

## 6. LLD 执行边界与技术契约 (LLD Constraints & Checkbox)
为指导下阶段的详细设计 (LLD) 与具体编码，必须满足以下技术契约：
- [ ] **环境变量前缀规范**: 明确定义注入环境变量的命名规范（例如：编译期参数统一使用 `COWEN_BUILD_*`，运行时全局覆盖使用 `COWEN_GLOBAL_*`）。
- [ ] **Trait 签名约束**: `Resettable` Trait 必须支持返回将要删除的资源清单 (用于 Dry Run)，例如 `async fn dry_run(&self) -> Vec<String>` 与 `async fn reset(&self) -> Result<()>`。
- [ ] **迁移日志标准**: 配置迁移脚本执行时，必须输出标准化的日志，明确告知用户哪些配置项被“上移”至了全局环境，哪些 Profile 配置被废弃。
