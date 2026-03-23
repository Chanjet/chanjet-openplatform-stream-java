# Module: connector-bom

## 1. 模块领域
本模块为 Maven **Bill of Materials (BOM)**，不包含任何业务代码。它是整个 Stream Gateway Java 生态的版本控制中心。

## 2. 能力范围
- 统一管理所有内部子模块的版本号。
- 集中定义第三方依赖（Spring Boot, Spring Cloud, Redis, JaCoCo 等）的版本。
- 确保整个多模块项目在构建时依赖的一致性，防止“依赖地狱”。

## 3. 准入规范
- **适合加入**: 第三方库的版本定义、新模块的声明。
- **严禁加入**: 任何 Java 类、资源文件或构建插件逻辑。
