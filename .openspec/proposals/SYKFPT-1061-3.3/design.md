# Design: Backpressure & Circuit Breaker (SYKFPT-1061-3.3)

## 1. 背压与限流架构 (Architecture)

### 1.1 并发控制模型
系统采用“双层守门员”模型：
- **门卫 1 (Node Guard)**: 保护机器物理内存。
- **门卫 2 (Tenant Guard)**: 确保租户隔离。

### 1.2 熔断状态机
- **CLOSED**: 正常转发。
- **OPEN**: 拦截所有请求，直接返回 503。
- **HALF_OPEN**: 放行少量探测请求（后续版本演进支持）。

## 2. 核心类设计

### 2.1 `IResilienceManager` (API 接口)
- `AcquisitionResult tryAcquire(String appKey)`: 尝试获取执行许可。
- `void release(String appKey, boolean success)`: 释放许可并反馈执行结果（用于计算失败率）。

### 2.2 `InMemResilienceManager` (核心实现)
- 使用 `ConcurrentHashMap<String, AtomicInteger>` 维护租户级计数。
- 使用一个全局 `AtomicInteger` 维护节点级计数。

## 3. TDD 测试计划
- `shouldDenyRequestWhenNodeLimitExceeded()`: 验证全局背压触发。
- `shouldDenyRequestWhenTenantLimitExceeded()`: 验证租户限流触发。
- `shouldEnterCircuitBreakerWhenFailureRateHigh()`: 验证熔断开启逻辑。
