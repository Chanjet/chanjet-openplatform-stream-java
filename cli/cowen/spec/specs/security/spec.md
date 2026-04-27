# Security Specification

## Capability: Webhook Security

### Requirement: 强制回环监听校验
WHEN 系统尝试启动任何本地 HTTP 监听服务（Proxy 或 OAuth2 Callback）时,
系统 SHALL 校验绑定地址是否为回环地址 (127.0.0.1 或 ::1)。

#### Scenario: 绑定公网 IP 失败
GIVEN 用户配置监听地址为 "0.0.0.0" 或 "192.168.1.5"
WHEN 调用监听启动函数
THEN 系统 SHALL 返回 SecurityError 错误
AND 拒绝启动服务。

#### Scenario: 绑定回环地址成功
GIVEN 监听地址为 "127.0.0.1" 或 "::1"
WHEN 调用监听启动函数
THEN 系统 SHALL 正常启动监听服务。
