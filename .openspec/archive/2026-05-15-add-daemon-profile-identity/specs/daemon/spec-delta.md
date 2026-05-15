# Specification Delta: Daemon Process Identity

## ADDED Requirements

### Requirement: 进程标识可视化 (Process Identity Visibility)
WHEN 守护进程 (Daemon) 在后台启动时,
系统 SHALL 自动将该进程的系统显示名称设置为 `cowen:<profile>` 格式。

#### Scenario: 启动带有 Profile 标识的进程
GIVEN 存在一个名为 "prod" 的 Profile
WHEN 执行 `cowen -p prod daemon start`
THEN 产生后台子进程
AND 该子进程在系统进程列表 (ps/top) 中的名称显示为 `cowen:prod`。

#### Scenario: 平台兼容性 (Fallback)
GIVEN 运行环境为不支持修改进程名的操作系统 (如 Windows)
WHEN 执行守护进程启动
THEN 系统 SHALL 保持默认进程名 `cowen` 且不应报错或异常退出。
