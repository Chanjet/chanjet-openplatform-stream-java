# 畅捷通 Stream Gateway 协议规范文档 v0.1.0

## 1. 握手与鉴权协议 (Handshake & Auth)

网关采用基于 Nonce 的挑战-应答机制，确保 ISV 的 `AppSecret` 永远不离开其本地环境。

### Step 1: 获取 Nonce (REST)

ISV 客户端首先请求网关获取一个具有时效性的 Nonce。

- **URL**: `GET /v1/ws/challenge?app_key={AppKey}`
- **Request Header**:
    - `X-CJT-PreAuth`: `HMAC_SHA256(app_key, AppSecret).hex().toLowerCase()[:16]` (用于防 DDoS)
- **Response 200 (OK)**:
    ```json
    {
      "code": "GW-0000",
      "data": {
        "nonce": "random-uuid-v4",
        "expires_in": 30
      }
    }
    ```

### Step 2: 建立 WebSocket 连接

客户端使用 Nonce 和签名建立连接。

- **URL**: `wss://{gateway_host}/connect?app_key={AppKey}&nonce={nonce}&sign={sign}&client_id={client_id}`
- **Query Params**:
    - `app_key`: 应用标识。
    - `nonce`: Step 1 获取的 Nonce。
    - `sign`: `HMAC_SHA256(app_key + "&" + nonce, AppSecret).hex().toLowerCase()`
    - `client_id`: 客户端实例 ID (建议格式：`{app_key}@{hostname}#{pid}`)。

### Step 3: 建连成功通知帧 (Server -> Client)

连接升级成功后，网关主动下发确认帧，ISV 客户端收到此帧后方可视为连接就绪。

```json
{
  "msg_type": "system",
  "event": "connected",
  "client_id": "{client_id}",
  "server_time": 1704067200123,
  "ping_interval": 10000
}
```

## 2. 消息下发协议 (Gateway -> Client)

网关将接收到的 Webhook 原封不动透传给客户端。

```json
{
  "msg_type": "event",
  "msg_id": "网关生成的UUID",
  "trace_id": "Core 侧透传的 TraceID",
  "timestamp": 1704067200000,
  "headers": {
    "X-C-APP_KEY": "...",
    "X-C-ORG_ID": "...",
    "X-MSG-ID": "Core 侧原始消息ID",
    "Content-Type": "application/json"
  },
  "payload": "{"biz_data":"..."}"
}
```

**关键点**：`payload` 必须是字符串（Raw Body），以便客户端直接进行 HMAC 签名比对，防止 JSON 重新序列化。

## 3. ACK 应答协议 (Client -> Gateway)

客户端必须在 **3 秒内** 返回确认帧。

```json
{
  "msg_type": "ack",
  "msg_id": "必须与推送消息的 msg_id 一致",
  "code": 200,
  "message": "success",
  "timestamp": 1704067200500
}
```

- **ACK 处理逻辑**：
    - `code == 200`: 网关返回 HTTP 200 给 Core。
    - `code != 200`: 网关返回 HTTP 503 给 Core。
    - `超时 (3s)`: 网关返回 HTTP 504 给 Core。

## 4. 心跳协议 (Keep-Alive)

为了穿透各种 LB 和防火墙，网关维护应用级心跳。

- **Gateway -> Client**: 发送 `{"msg_type":"ping"}`，每 10s 一次。
- **Client -> Gateway**: 收到 ping 后，必须在 5s 内回 `{"msg_type":"pong"}`。
- **断连触发**: 20s 内无任何消息往返，网关将强制切断连接。

## 5. 错误码规范

| Code | 状态 | 含义 |
| --- | --- | --- |
| **401** | Unauthorized | Sign 签名错误，客户端应停止重连并告警。 |
| **403** | Forbidden | AppKey 无权限或已禁用。 |
| **410** | Gone | Nonce 过期或已被使用。 |
| **429** | Too Many Requests | 触发频率限制。 |
| **503** | Service Unavailable | 网关节点过载或正在关闭。 |
