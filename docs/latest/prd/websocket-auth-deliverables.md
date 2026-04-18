# WebSocket 鉴权交付文档 (Deliverables)

## 1. 接口文档 (API Documentation)

### 1.1 申请 Nonce 预校验 (Verify PreAuth)
用于网关在颁发 Nonce 前进行的轻量级身份确认，防止匿名攻击。

- **接口路径**: `POST /internal/v1/auth/verify-preauth`
- **请求方**: Stream Gateway
- **请求体 (JSON)**:
  ```json
  {
    "app_key": "<APP_KEY>",
    "pre_auth_prefix": "<PRE_AUTH_PREFIX>" // ISV 提供的 HMAC 前缀
  }
  ```
- **算法说明**: 
  - 系统使用 `HMAC_SHA256(app_key, AppSecret)` 计算完整 HMAC。
  - 验证 `pre_auth_prefix` 是否为该 HMAC 的前缀（或完全匹配）。
- **响应体 (JSON)**:
  ```json
  {
    "valid": boolean // true 为合法，false 为拒绝
  }
  ```

### 1.2 签名验证接口 (Verify Sign)
用于验证 ISV 的 WebSocket 握手签名。

- **接口路径**: `POST /internal/v1/auth/verify-sign`
- **请求方**: Stream Gateway
- **请求体 (JSON)**:
  ```json
  {
    "app_key": "<APP_KEY>",
    "nonce": "<NONCE>",
    "sign": "<SIGNATURE>"
  }
  ```
- **签名算法**: 
  - `HMAC_SHA256(app_key + "&" + nonce, AppSecret).hex().toLowerCase()`
- **响应体 (JSON)**:
  ```json
  {
    "valid": boolean // true 为合法，false 为拒绝
  }
  ```

---

## 2. 使用说明 (Usage Instructions)

### 2.1 鉴权流程
1. **Nonce 申请阶段**：
   - ISV 客户端提供 `app_key` 和 `pre_auth_prefix`。
   - `pre_auth_prefix` 计算方法：`HMAC_SHA256(app_key, AppSecret)` 的前 16 位或完整十六进制字符串。
   - 网关调用 `/verify-preauth` 验证身份。
   - 验证通过后，网关向 ISV 返回一个随机 `nonce`。

2. **WebSocket 握手阶段**：
   - ISV 客户端根据收到的 `nonce` 计算签名 `sign`。
   - `sign` 计算方法：`HMAC_SHA256(app_key + "&" + nonce, AppSecret)` 的完整十六进制字符串（小写）。
   - ISV 发起 WebSocket 连接，在 Header 或 URL 参数中携带 `app_key`, `nonce`, `sign`。
   - 网关拦截握手请求，调用 `/verify-sign` 验证签名。
   - 验证通过后，允许建立连接。

### 2.2 安全建议
- `AppSecret` 必须严格保密，仅由 ISV 后端或受保护的环境持有。
- `nonce` 建议具有时效性（例如 5 分钟内有效）且仅限使用一次（由网关侧控制）。
- 所有 `/internal/` 路径接口均为服务间通信，不直接暴露给公网。

---
**交付日期**: 2026-03-19
**版本**: v0.1.0
