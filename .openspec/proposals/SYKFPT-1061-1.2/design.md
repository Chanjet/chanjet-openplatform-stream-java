# Design: Java Parent & BOM (SYKFPT-1061-1.2)

## 1. 层次结构
- `services/gateway-java/pom.xml`: 聚合根，管理插件、资源、编译参数。
- `services/gateway-java/connector-bom/pom.xml`: 依赖版本池。

## 2. 关键配置
- **Compiler**: `source/target=21`。
- **Encoding**: `UTF-8`。
- **BOM 引入**: 子模块通过 `import` 范围引入 `connector-bom`。

## 3. 依赖库清单建议
- **Spring Boot**: 4.0.0-M1 (或根据最新可用版本调整)
- **Redis**: Spring Data Redis
- **Security**: Spring Security (用于接口防护)
- **Tests**: JUnit 5, AssertJ, Mockito
