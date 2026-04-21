# OAuth 2.0 标准接入文档 (RFC 6749 / 7636)

本文档面向接入授权服务的上游业务方，详细说明 `/oauth2/token` 标准端点的接入规范。本次升级引入了 **刷新令牌轮换 (Refresh Token Rotation)** 以及 **PKCE (Proof Key for Code Exchange)** 支持，并统一了令牌有效期。

## 1. 核心变更概览

| 特性 | 标准端点 (`/oauth2/token`) | 历史接口 (`/token`) | 说明 |
| :--- | :--- | :--- | :--- |
| **刷新令牌轮换** | **强制开启 (Rotation)** | 不开启 | 每次刷新都会颁发新的 Refresh Token |
| **PKCE 支持** | **支持 (RFC 7636)** | 不支持 | 增强移动端/单页应用授权安全性 |
| **Access Token 效期** | **2 小时 (固定)** | 动态 (最高 6 天) | 提升安全性，降低泄露风险 |
| **Refresh Token 效期** | **7 天 (固定)** | 动态 (最高 6 天) | 统一管理，支持长期离线访问 |

---

## 2. 授权码模式获取令牌 (authorization_code)

### 请求地址
`POST /oauth2/token`  
`Content-Type: application/x-www-form-urlencoded`

### 请求参数
| 参数 | 类型 | 是否必选 | 说明 |
| :--- | :--- | :--- | :--- |
| `grant_type` | String | 是 | 固定值 `authorization_code` |
| `client_id` | String | 是 | 应用的 AppKey |
| `code` | String | 是 | `/authorize` 接口返回的授权码 |
| `code_verifier` | String | 建议 | PKCE 验证码（若授权时使用了 PKCE 则必填） |

### 响应示例
```json
{
  "access_token": "<ACCESS_TOKEN>",
  "refresh_token": "<REFRESH_TOKEN>",
  "expires_in": 7200,             
  "refresh_expires_in": 604800,
  "scope": "basic",
  "user_id": "user_123",
  "org_id": "org_456"
}
```

---

## 3. 令牌刷新模式 (refresh_token)

本项目实现了 **RFC 6749 推荐的刷新令牌轮换 (Token Rotation)** 机制。

### 核心逻辑说明
1. **单次使用**: 原有的 `refresh_token` 在使用一次后即刻作废。
2. **新令牌颁发**: 每次刷新请求成功后，响应报文中都会返回一个新的 `refresh_token`。
3. **并发容错 (Grace Period)**: 为处理移动端网络闪断或并发抖动导致的获取失败，系统提供 **5 分钟的宽限期**。在宽限期内，使用刚好失效的旧令牌会返回最近一次生成的相同 Token 对。

### 请求参数
| 参数 | 类型 | 是否必选 | 说明 |
| :--- | :--- | :--- | :--- |
| `grant_type` | String | 是 | 固定值 `refresh_token` |
| `refresh_token` | String | 是 | 当前持有的有效刷新令牌 |
| `client_id` | String | 是 | 应用的 AppKey |

### 响应示例
> [!IMPORTANT]
> 接入方**必须**在每次刷新后更新本地存储的 `refresh_token`。

```json
{
  "access_token": "<NEW_ACCESS_TOKEN>",
  "refresh_token": "<NEW_REFRESH_TOKEN>", // 必须更新本地存储
  "expires_in": 7200,
  "refresh_expires_in": 604800
}
```

---

## 4. PKCE 接入规范 (RFC 7636)

PKCE (Proof Key for Code Exchange) 是对授权码模式的安全加固，**强烈建议移动端 (App) 和单页应用 (SPA)** 强制使用。

### 4.1 核心概念要求

- **code_verifier**: 一个高熵随机字符串。
    - 长度范围：43 - 128 字符。
    - 字符集：`[A-Z]`, `[a-z]`, `[0-9]`, `-`, `.`, `_`, `~`。
- **code_challenge**: 验证码挑战值。由 `code_verifier` 经过特定算法处理得到。
- **code_challenge_method**: 挑战码计算算法。建议使用 `S256`。

### 4.2 计算逻辑 (前端实现参考)

> [!TIP]
> 推荐在前端使用 Web Crypto API 进行计算，确保安全性与性能。

```javascript
// 1. 生成高熵随机字符串 (code_verifier)
function generateVerifier(length = 64) {
    const array = new Uint32Array(length);
    window.crypto.getRandomValues(array);
    return Array.from(array, dec => ('0' + dec.toString(16)).slice(-2)).join('')
        .match(/[A-Za-z0-9\-._~]/g).join('').substring(0, length);
}

// 2. 计算挑战码 (code_challenge)
async function generateChallenge(verifier) {
    const encoder = new TextEncoder();
    const data = encoder.encode(verifier);
    const hash = await window.crypto.subtle.digest('SHA-256', data);
    return btoa(String.fromCharCode(...new Uint8Array(hash)))
        .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''); // Base64URL 编码
}
```

### 4.3 接入流程

#### 第一阶段：发起授权 (Authorize)

在跳转至 `authsite` 授权页面时，在 URL 中附加 PKCE 相关参数。

**请求路径示例：**
```text
GET /v2/userAuth/authorize?
    response_type=code&
    client_id=<APP_KEY>&
    redirect_uri=<REDIRECT_URL>&
    scope=all&
    state=<STATE>&
    code_challenge=<CHALLENGE>&
    code_challenge_method=S256
```

#### 第二阶段：换取令牌 (Token Exchange)

在获得 `code` 后，向 `/oauth2/token` 发送 POST 请求。

**POST Body 示例：**
```text
grant_type=authorization_code
client_id=<APP_KEY>
code=<THE_CODE>
redirect_uri=<REDIRECT_URL>
code_verifier=<VERIFIER_STRING>
```

### 4.4 安全注意事项

> [!CAUTION]
> 1. **不可持久化存储 verifier**：`code_verifier` 应当仅在授权跳转前生成，并临时存储于 SessionStorage 中，换取令牌后立即销毁。
> 2. **匹配强制性**：如果授权时提交了 `code_challenge`，后端在换取令牌时将**强制校验** `code_verifier`。如果未传入或不匹配，将返回 `4001` 错误。

---

## 5. 常见错误代码

| 代码 | 描述 | 处理建议 |
| :--- | :--- | :--- |
| `4029` | refreshToken 已过期 | 引导用户重新登录授权 |
| `4007` | refresh_token 不正确 | 检查令牌是否已被置换，或已过 5 分钟宽限期 |
| `4006` | appKey 不匹配 | 检查 `client_id` 与令牌颁发者是否一致 |
| `4001` | PKCE 验证失败 | 检查 `code_verifier` 是否与授权时的挑战码匹配 |

---

## 6. 开发建议

1. **原子化存储**: 建议对令牌的更新操作进行原子化管理，确保多线程/多进程环境下始终能拿到最新的 `refresh_token`。
2. **容错重试**: 发生 4007 错误时，代表令牌已彻底失效（非网络抖动），应立即触发重新授权流程。
3. **安全存储**: Refresh Token 效期长达 7 天，必须在客户端（如 Keychain, SecureStorage）中加密存储。
