# Module: connector-api

## 1. 模块领域
本模块定义了网关的 **领域契约 (Domain Contract)** 和 **SPI (Service Provider Interface)**。它是解耦业务逻辑与物理实现（如 Redis, HTTP）的关键层。

## 2. 能力范围
- 定义存储契约：`IRouteStore`, `INonceStore`, `IFailStore`。
- 定义通讯契约：`IP2PClient`, `IConnectionManager`。
- 定义业务配置模型：`ConnectorProperties`。
- 定义领域异常：`ConnectorException` 体系。

## 3. 准入规范
- **适合加入**: 所有的 Interface 定义、配置 Record/Class、领域专属异常。
- **严禁加入**: 任何具体的物理实现（如带 `redisTemplate` 的类、带 `RestClient` 调用的类）。本模块应保持“纯净”，不感知外部中间件。
