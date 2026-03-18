# OpenSpec 提案：Redis 路由与 Nonce 存储实现 (SYKFPT-1061-4.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-4.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 4.1: Redis 路由与 Nonce 存储实现 |

---

## 1. 问题背景 (Context)
核心逻辑层已经定义了 `IRouteStore`, `INonceStore` 和 `IFailStore` 契约。为了使网关支持集群化部署并实现跨节点 P2P 转发，我们需要一个基于分布式存储的实现。Redis 凭借其毫秒级的响应速度和原生的过期机制（TTL），是该场景的最佳选型。

## 2. 目标 (Objectives)
- 创建 `connector-infra` Maven 子模块。
- 基于 **Spring Data Redis** 实现 `IRouteStore` (Set 结构存储路由)。
- 实现 `INonceStore` (String 结构，单次核销)。
- 实现 `IFailStore` (String 结构，SETNX 计时器)。
- **严格遵循 TDD**：使用 **TestContainers** 运行 Redis 集成测试。

## 3. 技术设计 (Technical Design)

### 3.1 Redis 数据结构设计
1.  **路由表 (`route:{AppKey}`)**: 
    - 数据结构: `Set`。
    - 内容: `{node_ip}:{port}:{client_id}`。
    - TTL: 60s (由网关节点定期续期)。
2.  **挑战码 (`nonce:{uuid}`)**:
    - 数据结构: `String`。
    - 内容: `{AppKey}`。
    - TTL: 30s。
3.  **失败计时器 (`fail_start:{AppKey}`)**:
    - 数据结构: `String`。
    - 内容: 毫秒级时间戳。
    - TTL: 1h。

### 3.2 模块依赖
- 依赖 `connector-api`。
- 依赖 `spring-boot-starter-data-redis`。
- 测试依赖 `testcontainers-redis`。

## 4. 实施计划 (Implementation Plan)
1.  **工程搭设**: 创建 `connector-infra` 模块并配置 `pom.xml`。
2.  **编写集成测试**: 使用 TestContainers 模拟 Redis 环境。
3.  **编码实现**: 编写 Redis 版的 Store 实现类。
4.  **异常转换**: 确保将 `RedisConnectionFailureException` 等转化为 API 领域的通用异常。

## 5. 验证策略 (Verification Strategy)
- **正确性验证**: 验证 Add/Get/Remove 逻辑在真实 Redis 上的行为。
- **并发验证**: 多个网关实例同时写入同一个 AppKey 的路由，验证 Set 结构的幂等与安全性。

---
**审批意见**：待评审。
