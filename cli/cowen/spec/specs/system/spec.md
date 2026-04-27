# Specification Delta: Windows Daemon Recovery

## ADDED Requirements

### Requirement: 守护进程功能性自愈
WHEN 系统检查守护进程状态时,
系统 SHALL 同时校验进程的存在性 (PID) 与功能的可达性 (Port)。

#### Scenario: 探测到挂死进程并重启
GIVEN PID 文件存在且进程活跃
AND 本地代理端口 (Proxy Port) 无响应 (Connection Refused/Timeout)
WHEN 执行状态检查 (status) 或 启动检查 (ensure_running)
THEN 系统 SHALL 强制结束该进程
AND 删除旧的 PID 文件
AND 重新启动守护进程。

#### Scenario: 正常运行不做处理
GIVEN PID 文件存在且进程活跃
AND 本地代理端口响应正常
WHEN 执行检查
THEN 系统 SHALL 保持现状。
