# Design: Redis-based Implementations (SYKFPT-1061-4.1)

## 1. 核心类职责设计

### 1.1 `RedisRouteStore`
- **Key 策略**: `route:{appKey}`。
- **Add**: `redis.opsForSet().add(key, routeString)`，并设置 `expire(key, 60, SECONDS)`。
- **Get**: `redis.opsForSet().members(key)`。
- **Remove**: `redis.opsForSet().remove(key, routeString)`。

### 1.2 `RedisNonceStore`
- **Key 策略**: `nonce:{uuid}`。
- **Create**: `redis.opsForValue().set(key, appKey, 30, SECONDS)`。
- **Verify**: `redis.delete(key)` 并检查返回值，确保单次核销。

### 1.3 `RedisFailStore`
- **Key 策略**: `fail_start:{appKey}`。
- **GetOrSet**: 
    - 使用 `SET appKey timestamp NX EX 3600`。
    - 若设置成功，返回当前时间；若失败，执行 `GET` 获取已有的时间戳。

## 2. 序列化方案
- **Key**: 使用 `StringRedisSerializer`。
- **Value**: 使用 `StringRedisSerializer` (路由字符串和 Nonce 均为简单文本，无需复杂的 JSON 序列化)。

## 3. 集成测试环境
- 使用 **TestContainers** 启动 `redis:7.2-alpine` 镜像。
- 继承 `BaseRedisIntegrationTest` 以共享容器资源。
