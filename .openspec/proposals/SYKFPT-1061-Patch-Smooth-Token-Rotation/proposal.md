# OpenSpec 补丁提案：内部 P2P 令牌平滑切换支持 (SYKFPT-1061-Patch-Token)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-Patch-Token |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **优先级** | 中 (Medium) |

---

## 1. 问题描述 (Problem)
当前的 `internal-token` 采用单值配置。在更换令牌并进行集群滚动更新时，新旧节点会因为持有不同的令牌而导致 P2P 转发互通失败（HTTP 401/403），影响系统的高可用性。

## 2. 解决方案 (Solution)
- **配置升级**: 将 `connector.internal-token` (String) 升级为 `connector.internal-tokens` (List<String>)。
- **校验逻辑改进**: 网关接收 P2P 请求时，验证 Header 中的 Token 是否存在于配置的令牌列表中。
- **发送逻辑**: 默认使用列表中的第一个令牌（Primary Token）进行发送。

## 3. 实施计划 (Implementation Plan)
1.  **Red (红)**: 编写测试用例，模拟配置两个有效 Token 时，分别携带其中一个请求是否都能成功。
2.  **Green (绿)**: 更新 `InternalAuthInterceptor` (或 WebhookController 中的校验逻辑) 和相关配置类。
3.  **Refactor (重构)**: 确保环境变量注入支持多值（如使用逗号分隔）。

## 4. 验证策略 (Verification Strategy)
- 模拟“旧 Token 节点”访问“新旧共存节点”，验证通过。
- 模拟“新 Token 节点”访问“新旧共存节点”，验证通过。
