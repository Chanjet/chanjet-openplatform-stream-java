# 开放平台接口集成规范 (Detailed API Integration)

本文档提供了 `cowen` CLI 与畅捷通开放平台（Chanjet Open Platform）交互接口的详细报文结构、鉴权要求及错误处理逻辑。

---

## 🔐 认证类接口 (Authentication)

### 1. 重新发送 AppTicket
用于在本地缓存失效或首次初始化时，强制平台向 Streaming 通道推送最新的 `AppTicket`。

- **URL**: `/auth/appTicket/resend`
- **Method**: `POST`
- **Headers**:
  - `appKey`: 应用唯一标识
  - `appSecret`: 应用密钥
- **Request Body**:
  ```json
  {}
  ```
- **Success Response**:
  ```json
  {
    "code": "200",
    "message": "success"
  }
  ```

### 2. 自建应用获取令牌 (Self-Built)
- **URL**: `/v1/common/auth/selfBuiltApp/generateToken`
- **Method**: `POST`
- **Headers**:
  - `appKey`: <APP_KEY>
  - `appSecret`: <APP_SECRET>
- **Request Body**:
  ```json
  {
    "appKey": "<APP_KEY>",
    "appSecret": "<APP_SECRET>",
    "appTicket": "<APP_TICKET>",
    "certificate": "<ENCRYPTED_CERT>",
    "authCertificate": "<ENCRYPTED_CERT>"
  }
  ```
- **Success Response**:
  ```json
  {
    "result": true,
    "value": {
      "accessToken": "<ACCESS_TOKEN>",
      "expiresIn": 7200
    }
  }
  ```

### 3. 商店应用获取应用令牌 (Store-App)
- **URL**: `/auth/appAuth/getAppAccessToken`
- **Method**: `POST`
- **Headers**:
  - `appKey`: 应用唯一标识
  - `appSecret`: 应用密钥
- **Request Body**:
  ```json
  {
    "appTicket": "<APP_TICKET>"
  }
  ```
- **Success Response**:
  ```json
  {
    "result": {
      "appAccessToken": "<APP_ACCESS_TOKEN>",
      "expireTime": 7200
    }
  }
  ```

### 4. 获取组织永久授权码 (Permanent Code)
用于将临时授权码 `TempAuthCode` 转换为持久化的永久码。

- **URL**: `/auth/orgAuth/getPermanentAuthCode`
- **Method**: `POST`
- **Headers**:
  - `appKey`: 应用唯一标识
  - `appSecret`: 应用密钥
- **Request Body**:
  ```json
  {
    "tempAuthCode": "<TEMP_CODE>",
    "appAccessToken": "<APP_ACCESS_TOKEN>"
  }
  ```
- **Success Response**:
  ```json
  {
    "permanentAuthCode": "<PERMANENT_CODE>"
  }
  ```

### 5. 标准 OAuth2 令牌交换 (PKCE)
- **URL**: `/oauth2/token`
- **Method**: `POST`
- **Content-Type**: `application/x-www-form-urlencoded`
- **Request Body (Authorization Code)**:
  ```text
  grant_type=authorization_code
  &client_id=<CLIENT_ID>
  &code=<AUTH_CODE>
  &redirect_uri=<REDIRECT_URI>
  &code_verifier=<PKCE_VERIFIER>
  ```
- **Request Body (Refresh Token)**:
  ```text
  grant_type=refresh_token
  &client_id=<CLIENT_ID>
  &refresh_token=<REFRESH_TOKEN>
  ```
- **Success Response**:
  ```json
  {
    "access_token": "<ACCESS_TOKEN>",
    "refresh_token": "<REFRESH_TOKEN>",
    "expires_in": 7200,
    "refresh_token_expires_in": 604800
  }
  ```

---

## 🛠️ 规约与治理接口 (Governance)

### 1. 拉取 OpenAPI 规约
- **URL**: `/v1/common/openapi/spec`
- **Method**: `GET`
- **Query Params**:
  - `category`: 规约分类 (如 `all`, `base`)
- **Headers**:
  - `openToken`: <ACCESS_TOKEN>
  - `appKey`: <APP_KEY>
- **Response**: 返回标准 OpenAPI 3.0 (YAML/JSON) 格式文档。

### 2. 获取接口白名单 (Dynamic Discovery)
用于获取当前应用在指定组织下有权访问的所有接口清单。

- **URL**: `/developer/api/apiPermissions/isv/open/getInterfaceList`
- **Method**: `GET`
- **Headers**:
  - `openToken`: <ACCESS_TOKEN>
  - `appKey`: <APP_KEY>
- **Query Params**:
  - `page`: 分页页码 (0-based)
  - `size`: 分页大小 (通常为 100)
- **Success Response**:
  ```json
  {
    "value": {
      "currentPage": 0,
      "totalPages": 1,
      "resultList": [
        {
          "interfaceName": "查询用户信息",
          "requestPath": "/v1/user/query",
          "requestHttpMethod": "GET",
          "openApi": "{ \"paths\": { \"/v1/user/query\": { ... } } }"
        }
      ]
    }
  }
  ```
- **处理逻辑**: `cowen` 会递归遍历所有分页，解析 `openApi` 字段（如果存在）或根据 `requestPath` 构建影子規约，实现动态 API 发现。

---

## 📊 遙测与监控 (Telemetry)

### 1. 遥测数据上报
- **URL**: `/v1/telemetry/events`
- **Method**: `POST`
- **Request Body**:
  ```json
  {
    "event": "cli_command_executed",
    "fingerprint": "<MACHINE_FINGERPRINT>",
    "app_key": "<APP_KEY>",
    "version": "0.3.0",
    "os": "macos",
    "arch": "aarch64",
    "timestamp": "2024-05-01T12:00:00Z",
    "payload": {
      "command": "api list",
      "duration_ms": 120,
      "success": true
    }
  }
  ```

---

## ⚠️ 错误处理与重试矩阵

| HTTP 状态码 | 平台错误码 | 含义 | CLI 采取动作 |
| :--- | :--- | :--- | :--- |
| 401 | `4007` | Token 无效/过期 | 自动尝试 `refresh_token` |
| 409 | - | 并发冲突 (如重发推送频率过高) | 触发指数退避 (Exponential Backoff) |
| 429 | - | 触发平台限流 | 进入休眠并记录审计日志 |
| 500 | `50003` | 内部配置错误 (AppKey 不匹配等) | 终止进程并提示用户检查配置 |

---

## 📦 OpenAPI Spec 使用规范

`cowen` 在处理 Spec 时遵循以下逻辑：

1. **缓存一致性**: 规约文件在本地按 MD5 命名存储。如果平台返回的 ETag 未变化，则跳过下载。
2. **位置参数映射**:
   - 如果路径定义为 `/v1/orders/{orderId}`。
   - 用户输入 `cowen api GET /v1/orders/ORD123`，CLI 会自动识别并将 `ORD123` 映射为 `orderId`。
3. **签名注入**: 
   - 自动检测规约中的 `security` 要求。
   - 注入 `openToken` (Bearer) 或 `appKey/appSecret` (Custom Header)。
