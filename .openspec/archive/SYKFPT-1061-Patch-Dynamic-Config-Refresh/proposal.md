# OpenSpec 补丁提案：配置动态刷新支持 (SYKFPT-1061-Patch-Refresh)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-Patch-Refresh |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **优先级** | 高 (High) |

---

## 1. 问题描述 (Problem)
目前的 `ConnectorProperties` 采用 Java Record 实现，无法感知 Nacos 动态配置的变更。这意味着更换 `internal-tokens` 或修改限流阈值必须通过重启服务才能生效，增加了运维成本。

## 2. 解决方案 (Solution)
- **模型重构**: 将 `ConnectorProperties` 从 Java Record 降级为普通的 Java Class (POJO)，并提供 Setter 方法。
- **作用域升级**: 在配置类上增加 `@RefreshScope` (由 Spring Cloud 提供) 或确保其能被 `ConfigurationPropertiesRebinder` 捕获。
- **验证**: 通过单元测试模拟 `Environment` 变更，验证属性是否已实时刷新。

## 3. 实施计划 (Implementation Plan)
1.  **重构类结构**: 修改 `ConnectorProperties.java`。
2.  **Red (红)**: 编写测试用例，模拟配置对象在运行时值的变化。
3.  **Green (绿)**: 实现 POJO 结构。

## 4. 验证策略 (Verification Strategy)
- 修改 `ConnectorProperties` 实例的值，验证注入该实例的 Controller 是否同步感知。
