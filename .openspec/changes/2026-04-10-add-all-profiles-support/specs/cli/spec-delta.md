# 规范差异：CLI 全局操作支持

本文件包含对 `spec/specs/cli/spec.md` 的规范变更。

## MODIFIED 需求

### Requirement: 查看系统状态
**Previous**：用户使用 `cowen status` 命令只能查看当前激活环境的系统诊断信息。

用户可以通过 CLI 检查系统状态。
WHEN 用户执行 `cowen status`，
系统 SHALL 输出当前 Profile 的配置、凭证、Token 及守护进程状态。
WHEN 用户执行 `cowen status --all`，
系统 SHALL 扫描并依次输出所有已配置的 Profile 的系统状态。

#### Scenario: 查看所有环境状态
GIVEN 系统配置了 "default" 和 "prod" 两个 Profile
WHEN 用户执行 `cowen status --all`
THEN 系统首先输出 "default" 的诊断信息
AND 接着输出 "prod" 的诊断信息

---

### Requirement: 守护进程生命周期管理
**Previous**：`cowen daemon start/stop/restart` 主要针对单一 Profile 设计。

用户可以通过 CLI 统一管理守护进程生命周期。
WHEN 用户执行 `cowen daemon [start|stop|restart]`，
系统 SHALL 针对当前 Profile 执行相应操作。
WHEN 用户执行 `cowen daemon [start|stop|restart] --all`，
系统 SHALL 扫描所有存在的 Profile 配置文件，并依次执行该生命周期操作（对于 start，仅启动未运行的；对于 stop，仅停止正在运行的）。

#### Scenario: 全局管理环境守护进程
GIVEN 存在 "default" 和 "prod" 两个环境配置
WHEN 用户执行 `cowen daemon restart --all`
THEN 系统扫描到这两个配置
AND 依次重启 "default" 守护进程和 "prod" 守护进程
