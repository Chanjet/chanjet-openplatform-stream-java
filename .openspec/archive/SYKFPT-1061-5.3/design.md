# Design: System Integration & Optimization (SYKFPT-1061-5.3)

## 1. 虚拟线程配置 (Virtual Threads)
在 Spring Boot 4 中，开启虚拟线程将使整个网络栈（Tomcat/Jetty, RestClient, TaskExecutor）自动适配 Loom。
```yaml
spring:
  threads:
    virtual:
      enabled: true
```

## 2. 依赖注入管理 (DI)

### 2.1 基础设施 Bean
- `IRouteStore`: `RedisRouteStore`
- `IAuthService`: `RemoteCjtCoreAdapter`
- `IResilienceManager`: `InMemResilienceManager`

### 2.2 核心逻辑 Bean
- `MessageDispatcher`: 采用构造函数注入所有 SPI 实现。

## 3. 安全增强：WebSocket 握手拦截
- **类名**: `AuthHandshakeInterceptor`
- **逻辑**:
    1. 提取 Query Params: `app_key`, `nonce`, `sign`。
    2. 校验 Nonce 是否存在且未过期 (`INonceStore.verifyAndConsume`)。
    3. 校验签名是否合法 (`IAuthService.verifySign`)。
    4. 均通过则允许 `beforeHandshake` 返回 `true`。

## 4. TDD 验证计划
- `shouldBootApplicationContext()`: 验证 Context 启动成功。
- `shouldInjectAllSpiImplementations()`: 检查 `MessageDispatcher` 的 Bean 注入情况。
- `shouldRejectHandshakeWhenSignatureIsInvalid()`: 模拟非法签名的 WS 连接。
