# Design: Core REST Client (SYKFPT-1061-4.2)

## 1. 技术选型与配置
- **HTTP Client**: `org.springframework.web.client.RestClient` (Spring 6.1+)。
- **线程模型**: 同步阻塞，充分利用 Java 21 虚拟线程在 I/O 等待时的自动挂起特性。
- **序列化**: Jackson (JSON)。

## 2. API 映射设计

### 2.1 签名验证 (Verify Sign)
- **POST** `/internal/v1/auth/verify-sign`
- **Request Body**:
  ```json
  {
    "app_key": "string",
    "nonce": "string",
    "sign": "string"
  }
  ```
- **Success Response**: `200 OK` (Body: `{"valid": true}`)。

### 2.2 推送控制 (Push Status)
- **PATCH** `/internal/v1/subscriptions/{appKey}/push-status`
- **Request Body**:
  ```json
  {
    "enabled": boolean
  }
  ```
- **Success Response**: `204 No Content`。

## 3. TDD 测试矩阵 (Integration)
- `shouldReturnTrueWhenCoreVerifiesSignSuccessfully()`: 模拟 Core 返回 200 {"valid":true}。
- `shouldReturnFalseWhenCoreDeniesSign()`: 模拟 Core 返回 200 {"valid":false}。
- `shouldThrowExceptionWhenCoreReturns500()`: 模拟服务器错误。
- `shouldUpdatePushStatusCorrectly()`: 验证 PATCH 请求的路径变量和 Body。
