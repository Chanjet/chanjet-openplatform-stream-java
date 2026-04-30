# 外部交互边界 (External Interfaces)

## 1. 存储后端交互 (Persistence Tier)
### MySQL / PostgreSQL / SQLServer
- **协议**: TCP/IP (Standard Port: 3306/5432/1433)
- **驱动要求**: 建议使用 TLS 加密连接（非强制）。
- **并发控制**: 使用 `SELECT ... FOR UPDATE` 或分布式锁表实现记录级竞态保护。

### Redis
- **协议**: RESP (Standard Port: 6379)
- **使用场景**: 用于缓存短效 Token 以及分布式信号量。

## 2. 开放平台 API (External APIs)

### 2.1 凭据获取与刷新 (Auth & Token)
- **自建应用 (Self-built App) (Verified)**:
  - **机制**: 使用原 appTicket 机制换取 openToken。
  - **换票路由**: `POST /v1/common/auth/selfBuiltApp/generateToken` (Verified)
  - **补发 Ticket**: `POST /auth/appTicket/resend` (Verified)
  - **关键字段**: `appKey`, `appSecret`, `appTicket`, `certificate` (Verified)
- **商店应用 (Store App) (Verified)**:
  - **核心机制**:
    - **基础流程**: 标准 OAuth2.0 (Auth Code + PKCE) 获取 `access_token` 与 `refresh_token`。
    - **长效维护**: 支持使用“用户永久授权码”与“企业永久授权码”在 `refresh_token` 失效时重获凭据。
  - **换票/刷新路由**: `POST /oauth2/token` (Verified)
  - **关键字段**: `grant_type`, `code`, `refresh_token`, `client_id`, `permanent_auth_code` (Planned)

### 2.2 业务接口转发 (API Proxy)
- **T+ OpenAPI 转发 (Verified)**:
  - **路由格式**: `POST /tplus/api/v2/{service}/{action}` (Verified)
  - **鉴权要求**: 自动注入 `open_token` 或 `access_token` 至 HTTP Header。

## 3. Webhook 回调入站 (Webhook Inbound)
- **身份校验**: 必须通过 HTTP Header 中的 `x-chanjet-signature` (Verified) 进行签名校验。
- **转发策略**: Cowen 仅负责消息去壳（解密/解包）与转发，不执行幂等性或乱序防御检查，相关逻辑由下游业务系统实现。

---
*溯源参考：[Cowen v0.2.x 快照](../../references/cowen-v02-snapshot.md#cowen-v02-snapshot) (Verified)*
