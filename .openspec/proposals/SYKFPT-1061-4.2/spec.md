# Spec: Core Service Interaction (SYKFPT-1061-4.2)

## 1. 通讯安全规范
- **User-Agent**: 必须包含 `Cjt-Stream-Gateway/{version}`。
- **Auth Header**: 网关调用 Core 必须携带内部授权令牌 `X-GW-Token`。

## 2. 性能规范
- **Connect Timeout**: 1 秒。
- **Read Timeout**: 3 秒。
- **Max Retries**: 2 次（仅限 GET/PATCH 幂等操作）。

## 3. 容错规范
- 当 Core 服务整体不可用时，`IAuthService.verifySign` 必须返回 `false` (即默认拒绝握手)。
- 推送状态切换失败应记录 ERROR 日志，并进行指数退避补偿重试（后续演进）。
