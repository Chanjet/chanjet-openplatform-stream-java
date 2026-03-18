# Design: Java Parent & BOM (SYKFPT-1061-1.2)

## 1. 层次结构
- `services/gateway-java/pom.xml`: 聚合根，管理插件、资源、编译参数。
- `services/gateway-java/connector-bom/pom.xml`: 依赖版本池。

## 2. 关键配置
- **Compiler**: `source/target=21`。
- **Encoding**: `UTF-8`。
- **BOM 引入**: 子模块通过 `import` 范围引入 `connector-bom`。

## 3. 核心依赖版本矩阵
- **Spring Boot**: 4.0.3
- **Spring Cloud Alibaba**: 2024.x (适配 Boot 4 的最新预览版或稳定版)
- **Nacos**: 2.4.x (作为注册中心与配置中心)
- **JDK**: 21 (LTS)
