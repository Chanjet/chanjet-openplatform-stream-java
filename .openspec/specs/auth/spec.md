# Auth Specification

## Requirements

### Requirement: 认证模式配置 (Auth Mode Config)
WHEN 系统加载 Profile 配置,
系统 SHALL 支持 `app_mode` 字段,
其取值范围 SHALL 为 `self-built` 或 `oauth2`。

#### Scenario: 默认值兼容
GIVEN 存量 0.1.x 的配置文件（无 `app_mode` 字段）
WHEN 系统读取该配置
THEN `app_mode` SHALL 默认为 `oauth2`。

### Requirement: 认证提供者抽象 (AuthProvider Abstraction)
系统 SHALL 通过 `AuthProvider` Trait 提供认证能力,
该接口 SHALL 支持 `get_token` 与 `refresh` 操作。

### Requirement: OAuth2 令牌存储模型 (OAuth2 Token Pair Model)
系统 SHALL 支持 `OAuth2TokenPair` 模型,
包含 `access_token`, `refresh_token`, `expires_at`, `refresh_expires_at` 字段。

### Requirement: 认证分发逻辑 (Auth Dispatching)
系统 SHALL 根据当前 Profile 的 `app_mode`,
将令牌获取请求分发至对应的 `AuthProvider` 实例。

### Requirement: OAuth2Provider 行为 (AuthProvider Impl)
系统 SHALL 实现 `OAuth2Provider`,
对接 `https://openapi.chanjet.com/oauth2/token` 端点。

### Requirement: PKCE 协议支持 (PKCE Protocol Support)
系统 SHALL 支持标准 PKCE (RFC 7636) 流程,
Verifier 为 64 字节随机字符串, Challenge 使用 S256 算法。

### Requirement: 并发刷新锁 (Concurrent Refresh Lock)
系统 SHALL 在发起网络令牌刷新请求前获取 Profile 级文件排他锁,
并在锁内执行 Double-Check 以防止重复刷新。

### Requirement: 令牌自动轮换 (Token Rotation)
每次通过 `refresh_token` 换取新令牌成功后,
系统 SHALL 自动将响应中的新 `refresh_token` 持久化至 Vault。

### Requirement: 宽限期弹性处理 (Grace Period Resilience)
WHEN 刷新请求并行发生且平台处于宽限期 (5min) 时,
系统 SHALL 能够接受重复的令牌对而不视为故障。

#### Scenario: 令牌过期自动重整
GIVEN 系统检测到 Access Token 过期
WHEN 调用 `get_token`
THEN 系统 SHALL 自动发起 Refresh Grant 请求
AND 成功后更新本地缓存与 Vault。

#### Scenario: 刷新完全失效
GIVEN Refresh Token 已过期 (4029) 或无效 (4007)
WHEN 尝试刷新
THEN 系统 SHALL 抛出会话失效异常, 引导用户执行 `owenc init`。

### Requirement: 本地回调监听 (Local Callback Listener)
系统 SHALL 能够在本地启动临时 HTTP 服务器以接收授权回调,
- 监听地址 SHALL 为 `127.0.0.1`。
- 监听端口 SHALL 支持随机分配 (Port 0)。
- 路径 SHALL 为 `/oauth2/callback`。

### Requirement: 指令级单次捕获 (One-shot Capture)
监听器 SHALL 在成功捕获一次 `code` 后自动触发服务器关闭,
不应对系统资源造成长期占用。

### Requirement: 授权会话持久化 (Auth Session Persistence)
系统 SHALL 支持 `AuthSession` 模型持久化至 Vault (key: `pending_auth_session`),
- 必须包含 `code_verifier`, `state`, `redirect_uri`。
- Session 有效期 SHALL 为 5 分钟 (300 字符)。

### Requirement: 自动换票触发 (Finalizer Trigger)
WHEN 检测到 Vault 中存在有效的 `code` 且 Access Token 缺失/过期时,
系统 SHALL 自动执行换票流程并将换票结果保存。

### Requirement: 引导式初始化 (Guided Initialization)
WHEN 用户运行 `cowen init` 且指定 `app_mode: oauth2` (或交互选择)
系统 SHALL 自动开启授权引导流程。

### Requirement: 可选模式选择 (Mode Selection)
`cowen init` 指令 SHALL 支持 `--app-mode` 参数,
或在交互输入中提供 `self-built` 与 `oauth2` 选项。

### Requirement: QR Code 渲染 (QR Code Rendering)
系统 SHALL 在授权流程启动时, 在终端渲染对应的 QR Code,
以便用户在移动设备上快速授权。

### Requirement: 授权超时管理 (Auth Timeout)
授权监听器 SHALL 在运行 5 分钟后自动超时退出,
并提示用户重新运行指令。

### Requirement: 初始化参数校验 (Init Parameter Validation)
WHEN `app_mode: oauth2`
`app_secret` 与 `certificate` 参数 SHALL 标记为 OPTIONAL，且系统应校验其不应被手动指定。
WHEN `app_mode: self-built`
`app_secret` 与 `certificate` 保持 REQUIRED。

### Requirement: 认证注销逻辑 (Auth Logout)
系统 SHALL 提供 `owenc auth logout` 指令 (别名为 `auth reset`),
用于清除当前 Profile 的所有动态凭证而不破坏基础配置。

#### Scenario: 成功执行注销
GIVEN 用户已登录且存在 AccessToken/RefreshToken
WHEN 执行 `owenc auth logout`
THEN 系统 SHALL 清除 Vault 中对应的 Token, Ticket 与 Session 记录
AND `app_key` 与 `app_mode` 等配置 SHALL 保持不变。

#### Scenario: 重置语义对齐
GIVEN 为保持语义一致性
WHEN 执行 `owenc auth reset`
THEN 其行为 SHALL 与 `owenc auth logout` 完全一致（非破坏性）。

#### Scenario: 全量重置区分
WHEN 执行顶层 `owenc reset`
THEN 系统 SHALL 物理删除整个 Profile 的配置文件与所有 Vault 密钥。

#### Scenario: 成功引导初始化
GIVEN 用户运行 `owenc init --app-mode oauth2`
WHEN 系统生成授权链接
THEN 系统 SHALL 弹出浏览器并渲染终端 QR Code
AND 成功后由 Finalizer 自动完成后续配置。
