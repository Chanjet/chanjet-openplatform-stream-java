# 缺陷记录: Windows 更新后进程挂死问题 (BUG-20260423-WINDOWS-UPDATE-STUCK)

## 1. 缺陷描述 (Symptoms)
在 Windows 系统执行更新并重启/恢复后，`cowen` 后台守护进程（Daemon）会出现“挂死”现象：
- 进程依然存在于任务管理器中，但不再响应任何 CLI 指令或代理请求。
- 重新执行 `cowen` 相关指令无法触发自愈。
- **必须手动手动结束进程**（kill process）后，方可重新正常启动并恢复功能。

## 2. 复现步骤 (Steps to Reproduce)
1. 在 Windows 环境下启动 `cowen daemon start`。
2. 保持程序运行。
3. 执行 Windows 系统更新（或触发系统休眠后强行唤醒等类似电源管理事件）。
4. 系统恢复后，尝试调用 `cowen api ...` 或 `cowen status`。
5. 观察到指令超时或提示无法连接到本地代理。

## 3. 影响范围 (Impact Assessment)
- **用户体验**: 用户需要具备一定的技术背景去任务管理器处理残留进程，降低了工具的“自动化”感知。
- **业务连续性**: 在无人值守的机器上，系统更新可能导致桥接服务持久性中断。

## 4. 技术原因初探 (Potential Root Causes)
- **PID 文件锁未释放**: Windows 在更新过程中可能未正常向子进程发送 `SIGTERM` 或 `SIGKILL`，导致 PID 文件残留在磁盘。
- **僵尸进程/悬挂进程**: 进程可能处于挂起状态但未完全退出，依然占用了 `%USERPROFILE%/.owenc/default_daemon.pid` 或监听端口。
- **sysinfo 检测误判**: 当前 `system.rs` 仅通过 PID 是否存在来判断进程活跃度。在 Windows 更新后，PID 虽在但进程可能已失去响应能力。

## 5. 建议修复方案 (Proposed Solutions)
- **功能性状态检查 (Functional Health Check)**: 在 `ensure_daemon_running` 中增加对端口响应的探测，而非仅仅依赖 PID 检查。
- **引入守护进程“心跳”机制**: 守护进程定期更新 PID 文件中的时间戳或响应探测。
- **优化 Windows 信号处理**: 针对 Windows 特有的控制事件（如 `CTRL_SHUTDOWN_EVENT`）进行适配。

---
> [!NOTE]
> 该 Bug 已记录，计划在后续版本（v0.2.x 或 v0.3.0）中统一修复。
