# Proposal: Windows 守护进程挂死探测与自愈 (BUG-20260423)

## Why
在 Windows 系统执行更新或休眠恢复后，`cowen` 后台守护进程可能处于“挂死”状态（PID 存在但无响应）。目前的 `ensure_daemon_running` 仅依赖 PID 检查，无法发现并自愈此类功能性故障，导致服务持久性中断。

## What Changes
1. 在 `src/cmd/system.rs` 中新增 `is_port_responsive(port: u16)` 算子，通过 TCP 连接探测服务真实状态。
2. 优化 `ensure_daemon_running` 逻辑：如果 PID 存在但端口探测失败，则判定为 Hanging，执行强制清理并重启。
3. 实现自愈逻辑：清理旧的 PID 文件和僵死进程，重新调用 `daemon::start`。
4. 针对 Windows 平台引入更精细的状态检测（由于 Windows 下 PID 复用和挂起行为较多）。

## Impact
- **Reliability**: 显著提升 Windows 平台下的服务稳定性，支持异常后的自动恢复。
- **UX**: 用户无需手动进入任务管理器结束挂死进程。
