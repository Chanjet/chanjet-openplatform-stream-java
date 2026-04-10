# 提案：增加支持所有 Profile 的管理命令

## Why

**背景**：
- 目前 CLI 支持多 Profile 环境隔离（如 default, inte, prod）。
- 用户在日常维护时，经常需要同时查看所有环境的状态，或在升级版本后重启所有环境的守护进程。

**当前状态**：
- `cowen status` 仅支持输出当前激活的 Profile 的状态。
- `cowen daemon restart` 默认仅重启当前激活的 Profile（虽已定义 `--all` 参数，但逻辑需进一步完善以涵盖全局扫描）。
- 用户需要手动切换环境变量或使用 `--profile` 参数逐一操作，效率低下。

**期望状态**：
- `cowen status --all` 可以一次性格式化输出所有存在的 Profile 状态。
- `cowen daemon start --all` 可以一键启动所有尚未运行的 Profile 守护进程。
- `cowen daemon stop --all` 可以一键停止所有正在运行的 Profile 守护进程。
- `cowen daemon restart --all` 可以一键扫描并重启所有存在运行记录的 Profile 守护进程。

## What Changes

- 修改 CLI 参数定义，为 `Status` 命令添加 `--all` 布尔标志。
- 修改 CLI 参数定义，为 `DaemonCommands` 中的 `Start` 和 `Stop` 添加 `--all` 布尔标志（`Restart` 已有该参数，需补全逻辑）。
- 更新 `system::status` 和 `daemon::[start|stop|restart]` 的内部逻辑，当启用 `--all` 时：
  - 遍历配置文件目录（`~/.cowen/*.yaml`）。
  - 对每个找到的 Profile 依次执行操作。
- 优化多 Profile 的终端输出排版，确保信息展示清晰。

## Impact

### 受影响的规范
- `spec/specs/cli/spec.md` - 新增 `--all` 参数的行为规范。

### 受影响的代码
- `cli/cowen/src/main.rs` - 增加 `--all` 参数解析。
- `cli/cowen/src/cmd/system.rs` - 扩展 `status` 函数的入参和遍历逻辑。
- `cli/cowen/src/cmd/daemon.rs` - 确保 `restart` 函数正确处理多环境扫描。

### 用户影响
- 显著提升多环境维护效率，向下兼容旧行为。

### API 变更
- CLI 命令接口变更：`cowen status [--all]`。

### 需要迁移
- [x] 文档更新

## 时间线评估

小（0.5天）

## 风险

- 批量重启守护进程可能导致瞬间的端口争用或 CPU 峰值。
  - **缓解方案**：在循环重启时加入微小的延迟（如 500ms），避免雷群效应。
