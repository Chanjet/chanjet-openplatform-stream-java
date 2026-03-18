# Design: 30min Tolerance State Machine (SYKFPT-1061-3.2)

## 1. 状态机模型 (State Machine)

### 1.1 状态定义
- **ACTIVE**: 至少有一个在线客户端。
- **WAITING**: 零在线客户端，处于 30 分钟容忍期内。
- **SUSPENDED**: 零在线客户端，已通知 Core 挂起推送。

### 1.2 关键 API 契约扩展 (针对 TDD)
为了支持时间维度的 TDD，我们将引入 `ITimeProvider` 接口（或在测试中使用 Mock）。

## 2. 逻辑实现细节

### 2.1 失败处理 `handleFailure(String appKey)`
1. `long failStart = failStore.getOrSetNow(appKey)`。
2. `if (now - failStart >= 30min)`:
    - `pushControl.setPushEnabled(appKey, false)`。
    - `failStore.clear(appKey)`。
    - `return Result.SUSPENDED`。
3. `return Result.WAITING`。

### 2.2 重连处理 `handleReconnect(String appKey)`
1. `failStore.clear(appKey)`。
2. `pushControl.setPushEnabled(appKey, true)` (幂等调用)。

## 3. TDD 测试矩阵
- `shouldStartTimerOnFirstFailure()`: 验证第一次失败时是否创建了 Redis 键。
- `shouldReturn503WithinTolerancePeriod()`: 验证在 29 分 59 秒时仍返回等待状态。
- `shouldDisablePushWhenTolerancePeriodExpires()`: 验证第 31 分钟时触发了禁用指令。
- `shouldEnablePushOnClientReconnect()`: 验证重连时恢复推送指令。
