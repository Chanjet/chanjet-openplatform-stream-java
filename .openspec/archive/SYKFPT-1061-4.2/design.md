# Design: Core REST Client (SYKFPT-1061-4.2)

## 1. 技术选型与配置
- **HTTP Client**: `org.springframework.web.client.RestClient` (Spring 6.1+)。
- **线程模型**: 同步阻塞，充分利用 Java 21 虚拟线程在 I/O 等待时的自动挂起特性。
- **序列化**: Jackson (JSON)。

## 1. 架构定位：基础设施适配器 (Adapter)
本模块实现 `connector-api` 定义的契约，充当领域层与外部微服务之间的“转换器”。

## 2. 微服务发现与路由
API 调用细节封装在 `RemoteCjtCoreAdapter` 中，领域层对其不可见。

| 契约接口 | 映射 ServiceId | 映射 API 路径 |
| :--- | :--- | :--- |
| `IAuthService` | `${services.auth.id}` | `/internal/v1/auth/verify-sign` |
| `IPushControl` | `${services.subscription.id}` | `/internal/v1/subscriptions/{appKey}/push-status` |

## 3. 技术实现 (Infra 层)
- **解耦机制**: 领域层通过 `@Autowired IAuthService` 调用，Infra 层通过 `RestClient` + `ServiceId` 提供实现。
- **LoadBalancer**: 集成 `Spring Cloud LoadBalancer` 实现透明的微服务发现。

## 4. TDD 测试矩阵 (Integration)
- `shouldResolveServiceIdAndVerifySignSuccessfully()`: 模拟负载均衡成功并返回验证结果。
- `shouldHandleServiceNotFoundException()`: 模拟 Nacos 中找不到对应的微服务。

