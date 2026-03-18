# OpenSpec 提案：畅捷通 Core REST 客户端实现 (SYKFPT-1061-4.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-4.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 4.2: 畅捷通 Core REST 客户端实现 |

---

## 1. 问题背景 (Context)
网关在处理 WebSocket 握手时需要验证 ISV 的签名，在发现客户端离线或上线时需要通知 Core 服务挂起或恢复 Webhook 推送。这些操作依赖于畅捷通 Core 提供的内部 REST 接口。网关需要一个高可靠的 HTTP 客户端来实现这些契约。

## 2. 目标 (Objectives)
- 实现 `IAuthService` 接口，代理 Core 验证签名。
- 实现 `IPushControl` 接口，控制 Webhook 推送状态。
- 使用 Spring 6.1+ 引入的 **RestClient**（同步阻塞式，适配虚拟线程）。
- **严格遵循 TDD**：使用 **WireMock** 模拟 Core 服务进行集成测试。

## 3. 技术设计 (Technical Design)

### 3.1 核心组件
1.  **`CjtCoreRestClient`**:
    - 负责底层的 HTTP 请求封装。
    - 统一处理 API Token (网关自身的身份验证) 和超时逻辑。
2.  **接口对接**:
    - `verify-sign`: 发送 `app_key`, `nonce`, `sign` 到 Core 进行全量校验。
    - `push-status`: 更新特定 AppKey 的推送开关。

### 3.2 错误处理
- 自动处理 HTTP 4xx/5xx 响应，将其转化为逻辑层可识别的领域异常。
- 引入简易的重试机制（对于幂等的 Read 操作）。

## 4. 实施计划 (Implementation Plan)
1.  **编写集成测试**: `CjtCoreClientIT`。使用 WireMock 录制预期的 API 响应。
2.  **编码实现**: 编写基于 `RestClient` 的适配代码。
3.  **配置集成**: 在 `application.yml` 中定义 Core 服务的 BaseURL。

## 5. 验证策略 (Verification Strategy)
- **模拟校验成功/失败**: 验证 `IAuthService` 能否根据 WireMock 的返回正确给出布尔值。
- **超时验证**: 验证当 Core 服务响应慢时，网关能否正确触发超时并快速失败。

---
**审批意见**：待评审。
