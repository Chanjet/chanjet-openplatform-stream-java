# Design: Core REST Client (SYKFPT-1061-4.2)

## 1. 技术选型与配置
- **HTTP Client**: `org.springframework.web.client.RestClient` (Spring 6.1+)。
- **线程模型**: 同步阻塞，充分利用 Java 21 虚拟线程在 I/O 等待时的自动挂起特性。
- **序列化**: Jackson (JSON)。

## 2. 路由与 Service ID 配置
网关通过 Nacos 发现目标服务，`ServiceId` 均通过配置文件动态注入。

| 能力名称 | 对应 API 路径 | 默认 Service ID |
| :--- | :--- | :--- |
| **签名验证** | `/internal/v1/auth/verify-sign` | `cjt-auth-service` |
| **推送控制** | `/internal/v1/subscriptions/{appKey}/push-status` | `cjt-subscription-manager` |

## 3. 实现技术细节
- **LoadBalancer**: 使用 `Spring Cloud LoadBalancer` 拦截 `RestClient` 请求。
- **配置化**: 采用 `@Value("${services.auth.id}")` 和 `@Value("${services.subscription.id}")`。
- **统一 Client**: 抽象 `MicroserviceClient` 基础类，统一处理服务发现失败和熔断逻辑。

## 4. TDD 测试矩阵 (Integration)
- `shouldResolveServiceIdAndVerifySignSuccessfully()`: 模拟负载均衡成功并返回验证结果。
- `shouldHandleServiceNotFoundException()`: 模拟 Nacos 中找不到对应的微服务。

