# Design: SPI Contract Design (SYKFPT-1061-2.2)

## 1. 核心接口定义 (Core Interfaces)

### 1.1 IRouteStore
- `void add(String appKey, String nodeId, String clientId)`: 注册物理路由。
- `Set<RouteRecord> get(String appKey)`: 获取特定应用的所有活跃路由，支持集群负载均衡。
- `void remove(String appKey, String nodeId, String clientId)`: 销毁物理路由。

### 1.2 IConnectionManager (WS 传输层抽象)
- `boolean push(String clientId, EventFrame frame)`: 推送业务数据。
- `void close(String clientId, String reason)`: 强制断开指定连接。

### 1.3 IAuthService / IPushControl (Core 后台抽象)
- `boolean verifySign(String appKey, String nonce, String sign)`: 在线校验 ISV 签名。
- `void setPushEnabled(String appKey, boolean enabled)`: 动态开启或挂起特定 AppKey 的 Webhook 推送。

## 2. 异常处理 (Error Contract)
- 接口方法抛出的异常应限制在 `com.chanjet.connector.api.exception` 范围内，主要包括 `StoreException`, `RemoteAuthException`, `ConnectionException`。

## 3. 依赖传递
- 所有接口方法均应直接使用生成的 `Protobuf` 类或 `connector-common` 中的 Record 对象作为参数和返回值，实现全流程的强类型定义。
