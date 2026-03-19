# OpenSpec 提案：Webhook HTTP 接收器 (SYKFPT-1061-5.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-5.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 5.2: Webhook HTTP 接收器 |

---

## 1. 问题背景 (Context)
网关作为 Webhook-to-WebSocket 的桥接器，必须暴露一个高性能的 HTTP 接口，用于接收来自畅捷通 Core 服务（或内部转发节点）的 Webhook 请求。该接口需要提取元数据、解析 Payload，并调用领域层的 `MessageDispatcher` 进行路由分发。

## 2. 目标 (Objectives)
- 在 `connector-server` 中实现 `WebhookController`。
- 支持 `POST /internal/v1/webhook/dispatch` 接口。
- 提取关键 Headers (如 `X-C-APP_KEY`, `X-MSG-ID`, `X-Trace-Id`)。
- 封装 Body 到 `EventFrame` 并调用 `MessageDispatcher.dispatch`。
- **严格遵循 TDD**：使用 `MockMvc` 进行接口单元测试。

## 3. 技术设计 (Technical Design)

### 3.1 接口契约
- **Method**: `POST`
- **Path**: `/internal/v1/webhook/dispatch`
- **Required Headers**:
    - `X-C-APP_KEY`: 目标应用标识。
    - `X-MSG-ID`: 消息唯一 ID。
- **Optional Headers**:
    - `X-Trace-Id`: 链路追踪。
- **Body**: 原始业务文本/JSON。

### 3.2 逻辑流转
1.  **接收**: `WebhookController` 捕获 POST 请求。
2.  **转换**: 将 Headers 和 Body 包装为 `EventFrame` Record。
3.  **分发**: 调用 `MessageDispatcher.dispatch(frame)`。
4.  **响应**: 
    - 成功: 返回 200 OK。
    - 无路由/限流: 由领域层异常处理器转换为对应的 503/429 状态码。

## 4. 实施计划 (Implementation Plan)
1.  **编写测试用例**: `WebhookControllerTest`。模拟不同 Header 组合的请求。
2.  **实现 `WebhookController`**: 处理请求映射与参数绑定。
3.  **全局异常处理**: 在 `server` 模块实现 `@RestControllerAdvice`，处理领域层抛出的 `NoOnlineClientException` 等。

## 5. 验证策略 (Verification Strategy)
- **参数验证**: 验证缺失 `X-C-APP_KEY` 时是否返回 400 错误。
- **转发验证**: 模拟成功请求，验证 `MessageDispatcher` 是否被正确调用。
- **异常验证**: 模拟领域层抛出异常，验证 HTTP 状态码映射是否准确。

---
**审批意见**：待评审。
