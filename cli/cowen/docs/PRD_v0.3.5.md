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
    *   **继承与重写逻辑**: 
        *   系统优先读取 `app.yaml` 中的全局配置。
        *   Profile 级别的配置仍可保留，但仅作为针对特定环境的**可选覆盖 (Override)**。
    *   **受影响模块**: `ConfigManager` 的寻址与合并算法需适配新的优先级权重。
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
    *   **CI 适配**: 更新 Makefile，支持在构建命令中通过变量传递这些参数。
*   **验收标准**: 
    *   在不修改源码的情况下，通过 `COWEN_BUILD_CLIENT_ID=XXXX make build` 产生的二进制，其内置 Client ID 为 `XXXX`。
    *   通过 `cowen version --debug` 或类似指令可验证内置参数的准确性。

### 2.3 补全 OCP 抽象：系统重置模块化 (Modular System Reset)
*   **需求背景**: 目前 `cowen reset` 逻辑高度过程化，散落在各命令实现中。为了符合开闭原则（OCP），新增存储或组件时需要能自动参与重置流程。
*   **功能描述**: 
    *   **抽象 Trait**: 定义 `Resettable` Trait。
    *   **自动注册**: 各核心组件（Vault, Store, Telemetry, Config）实现该 Trait 并通过插件化方式注册。
    *   **调度重构**: 重构 `cmd/reset.rs`，使其仅作为调度器遍历并执行所有已注册组件的重置方法。
*   **验收标准**: 
    *   新增一个伪组件并注册重置逻辑后，运行 `cowen reset` 必须能自动触发该逻辑，无需修改 `reset.rs` 主代码。

## 3. 技术设计与关键技术选项确认 (Technical Design & Technology Options)


### 3.1 配置合并优先级 (Merge Strategy)
*   **优先级权重**: `环境变量 (Highest)` > `命令行 Flag` > `Profile 覆盖配置 (Profile Config)` > `应用全局配置 (app.yaml)` > `内置编译默认值 (Lowest)`。
*   **实现方式**: 在 `ConfigManager::load` 中引入多级 Layer 合并逻辑，利用 `serde_json::Value` 或 `merge` 库实现深合并。

### 3.2 构建注入实现
*   **方案**: 使用 `build.rs` 输出 `cargo:rustc-env=KEY=VALUE`。
*   **代码调用**: 使用 `env!("KEY")` 宏获取编译期变量，并提供 fallback 默认值。

## 4. 影响范围评估 (Impact Assessment)
*   **向下兼容性**: 需提供配置自动迁移逻辑，在 v0.3.5 首次运行时，将现有默认 Profile 中的配置项平滑搬迁至全局 `app.yaml`。
*   **开发体验**: 减少了在 `init` 多个 Profile 时输入重复参数的需要。

## 5. 任务计划 (High-level WBS)
1. **P1.1**: 重构 `AppConfig` 结构并更新 `app.yaml` 读写逻辑。
2. **P1.2**: 修改 `ConfigManager` 合并算法，实现全局配置继承。
3. **P1.3**: 实现 `build.rs` 注入机制，清理 `config.rs` 硬编码项。
4. **P1.4**: 系统重置逻辑 OCP 模块化重构。
5. **P1.5**: 编写配置平滑迁移脚本，确保旧版本用户无感升级。
6. **P1.6**: 全量回归验证。
