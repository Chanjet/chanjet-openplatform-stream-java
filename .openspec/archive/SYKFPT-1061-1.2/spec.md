# Spec: Java Development Standards (SYKFPT-1061-1.2)

## 1. Maven 规范
- 所有子模块必须以 `com.chanjet.connector` 为 GroupId 前缀。
- 所有三方依赖版本严禁在子模块中直接定义，必须在 `connector-bom` 中声明。

## 2. 编码规范
- 严格遵循 Google Java Style Guide。
- 强制使用 Java 21 语法特性（Records, Pattern Matching）。
- 关键业务逻辑必须包含 Javadoc 注释。

## 3. 交付物规范
- 生成的 JAR 必须包含 `MANIFEST.MF` 元数据，如版本号、构建时间。
