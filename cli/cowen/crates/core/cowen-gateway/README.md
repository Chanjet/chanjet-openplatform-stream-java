# cowen-gateway

`cowen-gateway` 是 `cowen` 的核心组件之一，实现了零信任应用网关（Identity-Aware Gateway）的功能。

## 核心职能

1. **路由解析与匹配**：解析进入网关的请求，基于声明式路由策略（STRICT / LAX 模式）判定目标路由的安全属性与绕过规则。
2. **会话校验 (Zero-Trust Session)**：自动提取并验证 AES-256-GCM 加密的 session cookie，对 IP + User-Agent 进行 SHA-256 签名绑定（防 Cookie 劫持）。
3. **JWKS 密钥管理与自动轮转**：管理用于 JWT 签名的本地密钥集 `cowen:system:jwks`，支持 30 天自动密钥轮转，平滑过滤已轮转（ROTATED）的历史密钥。
4. **Code 拦截与 Wash 机制**：自动捕获并拦截平台重定向 callback 请求中的临时授权码 (`code`) 并与 ISV 服务端完成 token 洗刷同步。
5. **三级凭证恢复机制 (3-Tier Token Recovery)**：在 Egress 反向代理模式下，支持 `本地 Cache` -> `Refresh Token 刷新` -> `永久授权码 (Permanent Auth Code) 换取` 的三级凭证自愈恢复链路。
