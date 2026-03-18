# OpenSpec 提案：Java 父工程与 BOM 配置 (SYKFPT-1061-1.2)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-1.2 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 1.2: Java 父工程与 BOM 配置 |

---

## 1. 问题背景 (Context)
在多模块 Java 项目中，依赖版本不一致常导致运行时冲突（如 `NoSuchMethodError`）。为了确保 `gateway-java` 下的所有子模块（api, core, infra, server）使用统一且兼容的三方库版本，我们需要建立一个 Maven 父工程。

## 2. 目标 (Objectives)
- 创建 `gateway-java` 父 `pom.xml`。
- 创建 `connector-bom` 模块，集中管理依赖版本。
- 引入 Spring Boot 4 和 JDK 21 相关的依赖基础。

## 3. 技术设计 (Technical Design)

### 3.1 模块结构
```text
services/gateway-java/
├── pom.xml                # 父工程 (Parent POM)
└── connector-bom/         # 依赖版本管理模块 (BOM)
    └── pom.xml
```

### 3.2 依赖管理策略
- **Parent POM**: 定义编译插件版本（Maven Compiler Plugin for JDK 21）及共用配置（编码、资源过滤）。
- **BOM (Bill of Materials)**: 使用 `<dependencyManagement>` 锁定 Spring Boot, Jackson, Redis, JUnit 5 等核心库版本。

## 4. 实施计划 (Implementation Plan)
1.  **编写 Parent POM**: 设置 `<packaging>pom</packaging>`，并引用 `connector-bom`。
2.  **编写 connector-bom**: 声明所有预期的三方依赖版本。
3.  **根目录 Makefile 适配**: 更新 `build-java` 指令以适配多模块构建。

## 5. 验证策略 (Verification Strategy)
- **编译验证**: 在 `services/gateway-java` 下执行 `mvn clean install`。
- **依赖检查**: 运行 `mvn dependency:tree` 确保子模块可正确继承 BOM 中的版本。

---
**审批意见**：待评审。
