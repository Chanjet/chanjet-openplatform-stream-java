# Spec: Redis Storage Standards (SYKFPT-1061-4.1)

## 1. Key 命名规范
- 所有 Key 必须携带统一前缀（如 `cjt:gw:`），方便在共享 Redis 环境中进行审计和清理。
- 动态部分统一使用小写蛇形命名 (`snake_case`)。

## 2. 性能规范
- **响应时间**: 单次读写操作必须在 10ms 内完成 (P99)。
- **连接管理**: 强制使用连接池（Lettuce），并开启虚拟线程适配。

## 3. 可靠性规范
- **超时配置**: 读写超时时间统一设置为 500ms。
- **异常屏蔽**: 当 Redis 集群发生脑裂或完全宕机时，Store 实现层必须抛出 `com.chanjet.connector.api.exception.StoreException`，严禁让底层 `RedisSystemException` 逃逸到逻辑层。

## 4. 路由生存周期
- 路由记录的默认 TTL 为 60s。
- 连接维系节点负责每 20s 对 Key 执行一次 `EXPIRE` 续期。
