# Module: connector-infra

## 1. 模块领域
本模块负责 **物理落地 (Implementation)**。它为 `connector-api` 中定义的抽象契约提供基于具体中间件的物理实现。

## 2. 能力范围
- 存储实现：`RedisRouteStore`（基于 Redis 的路由存储）、`RedisNonceStore`。
- 微服务适配：`RemoteCjtCoreAdapter`（通过 RestClient 调用 Nacos 上的 Auth/Sub 服务）。
- 通讯实现：`RestP2PClient`（基于 HTTP 的节点间转发）。

## 3. 准入规范
- **适合加入**: 任何依赖第三方中间件（Redis, Spring Cloud, Apache HttpClient）的代码。
- **严禁加入**: 任何业务逻辑判断（如“什么时候应该重试”）。本模块只负责“执行”，即：按照 API 要求，把数据存进 Redis 或发给某个 URL。
