# 畅捷通 Stream Gateway 接口规范文档 v0.1.0

## 1. 外部接口 (ISV 面向)

### 1.1 获取 Nonce 挑战 (REST)
用于 WebSocket 握手前的身份预校验。

- **Endpoint**: `GET /v1/ws/challenge`
- **Query Params**:
  - `app_key` (Required): ISV 应用 Key。
- **Headers**:
  - `X-CJT-PreAuth`: `HMAC_SHA256(app_key, AppSecret).hex().toLowerCase()[:16]`
- **Response**:
  ```json
  {
    "code": "GW-0000",
    "message": "success",
    "data": {
      "nonce": "uuid-v4-string",
      "expires_in": 30
    }
  }
  ```

### 1.2 建立 WebSocket 连接 (WSS)
- **Endpoint**: `wss://{host}/connect`
- **Query Params**:
  - `app_key`: 应用标识。
  - `nonce`: 挑战码。
  - `sign`: `HMAC_SHA256(app_key + "&" + nonce, AppSecret).hex().toLowerCase()`
  - `client_id`: 客户端唯一标识。

## 2. 内部接口 (Core 面向)

### 2.1 接收 Webhook 转发 (Dispatch)
Core 通过此接口向网关投递 Webhook。

- **Endpoint**: `POST /internal/v1/webhook/dispatch`
- **Headers**:
  - `X-C-APP_KEY`: 目标应用 Key。
  - `X-MSG-ID`: 原始消息唯一 ID。
  - `X-Trace-Id`: 全链路追踪 ID。
- **Body**: 原始业务 JSON/Text Payload。
- **Success Response (200 OK)**:
  ```json
  { "result": "success" }
  ```
- **Error Response (503/504)**:
  ```json
  { "result": "error", "message": "ack_timeout/offline" }
  ```

## 3. 跨节点转发接口 (Node-to-Node)

### 3.1 节点间 P2P 转发
用于网关节点之间的 HTTP 转发。

- **Endpoint**: `POST /internal/v1/p2p/push`
- **Headers**:
  - `X-Internal-Target-Client-ID`: 目标客户端 ID。
  - `X-MSG-ID`: 原始消息 ID。
  - `X-Trace-Id`: 追踪 ID。
- **Body**: 原始业务 Payload 及 Headers 的 JSON 封装。

## 4. 依赖 Core 的外部验证接口 (Auth Proxy)

网关调用 Core 提供的验证接口，以实现 No-Secret 鉴权：

### 4.1 Nonce 申请预校验
- **Endpoint**: `POST /internal/v1/auth/verify-preauth`
- **Body**: `{"app_key": "...", "pre_auth_prefix": "..."}`
- **Response**: `{"valid": boolean}`

### 4.2 握手签名验证
- **Endpoint**: `POST /internal/v1/auth/verify-sign`
- **Body**: `{"app_key": "...", "nonce": "...", "sign": "..."}`
- **Response**: `{"valid": boolean}`

### 4.3 推送状态切换
- **Endpoint**: `PATCH /internal/v1/subscriptions/{app_key}/push-status`
