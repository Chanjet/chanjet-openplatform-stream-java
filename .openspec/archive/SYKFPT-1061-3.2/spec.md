# Spec: Tolerance & Push Control (SYKFPT-1061-3.2)

## 1. 核心参数规范
- **容忍时长 (Tolerance Duration)**: 固定为 1,800,000 毫秒 (30 分钟)。
- **计时器 TTL**: 3,600 秒 (1 小时)，确保容忍期结束后计时器能自动回收。

## 2. 推送控制契约 (Push Control Spec)
- **Action: DISABLE**: 网关发送此指令后，Core 必须停止向网关 Dispatch 路径发送任何 POST 请求。
- **Action: ENABLE**: 网关发送此指令后，Core 必须在 5 秒内启动积压消息的扫描并尝试补发。

## 3. 错误码映射
- 处于 **WAITING** 或 **SUSPENDED** 状态时，网关入口 Controller 统一向 Core 返回 `HTTP 503 Service Unavailable`。
- Core 接收到 503 后，应执行指数退避重试逻辑。
