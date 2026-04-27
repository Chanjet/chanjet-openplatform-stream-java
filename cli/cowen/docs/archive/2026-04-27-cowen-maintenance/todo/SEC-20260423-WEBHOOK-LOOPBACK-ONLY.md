# 安全策略: Webhook 服务仅限回环地址 (SEC-20260423-WEBHOOK-LOOPBACK-ONLY)

## 1. 风险描述 (Risk Description)
Webhook 接收端（或其他监听服务）如果监听在非回环 IP（如 `0.0.0.0` 或公网 IP）上，可能会导致以下安全风险：
- **凭据泄露**: 攻击者可能构造恶意的 Webhook 请求，尝试利用本地服务的漏洞或直接向其发送请求以获取敏感信息。
- **中间人攻击**: 在非加密且非回环的网络中，通信内容可能被拦截。
- **未授权访问**: 本地开发的 Webhook 服务通常不具备复杂的认证机制，如果暴露在局域网或公网，极易被非授权访问。

## 2. 核心原则 (Core Policy)
`cowen` CLI 及相关组件提供的一切“Webhook 监听”或“本地服务”必须遵循以下原则：
- **默认监听地址**: 必须硬编码或默认为 `127.0.0.1` (IPv4) 或 `::1` (IPv6)。
- **严禁外部监听**: 严禁在未经用户明确、多步操作授权的情况下，将 Webhook 服务绑定到非回环地址。
- **日志审计**: 在服务启动时，必须明确告知用户当前监听的地址和端口。

## 3. 当前实现检查 (Status Check)
- [x] **Local Proxy**: 已绑定至 `127.0.0.1` ([src/daemon/proxy.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/src/daemon/proxy.rs#L36))。
- [x] **OAuth2 Callback Listener**: 已绑定至 `127.0.0.1` ([src/auth/lifecycle/listener.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/src/auth/lifecycle/listener.rs#L36))。

## 4. 后续改进建议 (Proposed Hardening)
- **硬编码校验**: 在所有涉及监听的代码位置，增加一个强制性的回环地址校验逻辑，防止意外配置为 `0.0.0.0`。
- **文档说明**: 在配置项说明中，明确告知用户为何只支持 `localhost`，并说明如果需要跨机器测试时应使用的替代方案（如 SSH Tunnel）。

---
> [!IMPORTANT]
> 安全性是 `cowen` 的核心红线。所有 Webhook 转换和代理服务严禁向非回环 IP 提供服务。
