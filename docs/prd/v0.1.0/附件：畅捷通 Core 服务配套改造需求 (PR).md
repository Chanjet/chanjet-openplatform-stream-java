# 畅捷通 Core 服务配套改造需求 (PR v0.1.0)

> **文档说明**：本 PR 用于协调相关微服务（Auth / Subscription）配合 Stream Gateway 实现 WebSocket 模式下的安全接入与动态推送控制。

---

## 1. 鉴权服务改造 (Auth Service)

**目标**：支持网关代理验证 ISV 的 WebSocket 握手签名及 Nonce 申请权限。

### 1.1 申请 Nonce 预校验 (Verify PreAuth)
- **接口路径**: `POST /internal/v1/auth/verify-preauth`
- **请求体 (JSON)**:
  ```json
  {
    "app_key": "string",
    "pre_auth_prefix": "string" // ISV 提供的 HMAC 前缀
  }
  ```
- **响应体 (JSON)**: `{"valid": boolean}`
- **用途**: 用于网关在颁发 Nonce 前进行的轻量级身份确认，防止匿名攻击。

### 1.2 签名验证接口 (Verify Sign)
- **接口路径**: `POST /internal/v1/auth/verify-sign`
- **请求方**: Stream Gateway
- **请求体 (JSON)**:
  ```json
  {
    "app_key": "string",
    "nonce": "string",
    "sign": "string"
  }
  ```
- **签名算法**: `HMAC_SHA256(app_key + "&" + nonce, AppSecret).hex().toLowerCase()`
- **响应体 (JSON)**:
  ```json
  {
    "valid": boolean  // true 为合法，false 为拒绝
  }
  ```

---

## 2. 订阅/推送管理服务改造 (Subscription Manager)

**目标**：支持网关根据 ISV 客户端的在线状态，动态开启或挂起（挂载到离线池）消息推送。

### 2.1 推送状态控制接口 (Push Status)
- **接口路径**: `PATCH /internal/v1/subscriptions/{appKey}/push-status`
- **请求方**: Stream Gateway
- **请求体 (JSON)**:
  ```json
  {
    "enabled": boolean  // true: 开启推送; false: 挂起推送并进入离线积压池
  }
  ```
- **预期行为**:
    - 当 `enabled = false` 时，核心服务应停止向该 AppKey 对应的 Webhook 回调地址（即网关 Dispatch 地址）发送实时消息，转而存入离线池。
    - 当 `enabled = true` 时，核心服务应立即恢复实时推送，并启动后台任务补发离线池中的积压消息。
- **响应码**: 
    - `204 No Content` (成功)
    - `404 Not Found` (AppKey 无订阅记录)

---

## 3. 安全要求 (Security)
- 以上接口均为 **内部通信**，仅允许来自网关节点的请求。
- 网关会在 Header 中携带内部令牌 `X-GW-Token` 用于身份校验（待联调确定）。

---
**提交人**：畅捷通 Stream Gateway 架构组
**日期**：2026-03-19
