# Module: connector-server

## 1. 模块领域
本模块是整个项目的 **入口 (Shell)**。它集成了 Spring Boot 框架，负责管理所有 Bean 的生命周期，并暴露外部可访问的接口。

## 2. 能力范围
- HTTP 入口：`WebhookController` (分发入口), `ChallengeController` (Nonce 入口)。
- WebSocket 接入：`DefaultWsHandler` (Session 管理)、握手拦截器。
- 配置加载：多环境 Profile、运行时 NodeId 解析、属性解密集成。
- 运维监控：Spring Boot Actuator (健康检查端口 8081)。

## 3. 准入规范
- **适合加入**: Spring 配置类 (@Configuration)、HTTP 请求映射 (@RestController)、WebSocket 处理逻辑、全局异常处理。
- **严禁加入**: 核心业务算法（应去 core）、具体 Redis 操作（应去 infra）。本模块的角色是 **组装 (Composition)**。
