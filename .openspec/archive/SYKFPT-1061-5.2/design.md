# Design: Webhook HTTP Receiver (SYKFPT-1061-5.2)

## 1. 核心类设计 (Classes)

### 1.1 `WebhookController`
- **职责**: REST 接口入口。
- **依赖**: `MessageDispatcher` (Core 领域服务)。
- **逻辑**:
    - 使用 `@RequestHeader` 绑定元数据。
    - 使用 `@RequestBody` 接收原始报文。
    - 构建 `EventFrame`。
    - 调用 `dispatcher.dispatch(frame)`。

### 1.2 `GlobalExceptionHandler`
- **职责**: 将领域异常翻译为 HTTP 语义。
- **映射规则**:
    - `NoOnlineClientException` -> `503 Service Unavailable`。
    - `AcquisitionException (Limited)` -> `429 Too Many Requests`。
    - `InvalidParameterException` -> `400 Bad Request`。

## 2. 线程模型与并发
- **Virtual Threads**: 所有 HTTP 请求处理默认运行在 Java 21 虚拟线程上，允许在 P2P 转发时进行同步阻塞等待而无性能损耗。

## 3. TDD 测试矩阵 (MockMvc)
- `shouldReturn200WhenDispatchSucceeds()`: 验证正常分发流。
- `shouldReturn400WhenAppKeyIsMissing()`: 验证参数校验。
- `shouldReturn503WhenNoClientOnline()`: 验证领域异常转换。
