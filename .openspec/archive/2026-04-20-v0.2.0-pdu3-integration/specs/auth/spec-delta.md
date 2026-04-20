# Specification Delta: Cowen CLI v0.2.0 PDU 3 - Integration & UI/UX

## ADDED Requirements

### Requirement: 引导式初始化 (Guided Initialization)
WHEN 用户运行 `cowen init` 且指定 `app_mode: oauth2` (或交互选择)
系统 SHALL 自动开启授权引导流程。

### Requirement: 可选模式选择 (Mode Selection)
`cowen init` 指令 SHALL 支持 `--app-mode` 参数,
或在交互输入中提供 `self_built` 与 `oauth2` 选项。

### Requirement: QR Code 渲染 (QR Code Rendering)
系统 SHALL 在授权流程启动时, 在终端渲染对应的 QR Code,
以便用户在移动设备上快速授权。

### Requirement: 授权超时管理 (Auth Timeout)
授权监听器 SHALL 在运行 5 分钟后自动超时退出,
并提示用户重新运行指令。

## MODIFIED Requirements

### Requirement: 初始化参数校验 (Init Parameter Validation)
WHEN `app_mode: oauth2`
`app_secret` 与 `certificate` 参数 SHALL 标记为 OPTIONAL。
WHEN `app_mode: self-built`
`app_secret` 与 `certificate` 保持 REQUIRED。

#### Scenario: 成功引导初始化
GIVEN 用户运行 `owenc init --app-key <AK>` 且未提供 Secret
WHEN 系统识别到该应用可能支持 OAuth2
THEN 系统 SHALL 提示用户开始授权引导。
