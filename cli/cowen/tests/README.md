# Cowen CLI E2E 测试套件指南

本目录包含了 Cowen CLI 的端到端 (E2E) 自动化测试脚本，用于验证核心功能在不同授权模式、存储模式及分布式环境下的稳定性与健壮性。

## 🚀 如何运行

### 环境准备
- **Python 3**: 用于运行 Mock Server (`tests/mock_server.py`)。
- **SQLite3**: 验证数据库状态。
- **Cargo**: 用于编译待测二进制文件。
- **Redis**: 运行 `case_17` 和 `case_18` 需要本地 Redis 服务。

### 执行测试
运行所有测试用例：
```bash
bash tests/run_suites.sh
```

运行单个测试用例（例如 Case 28）：
```bash
cargo build && bash tests/case_28_store_app_multi_org_stress.sh
```

---

## 📊 测试覆盖范围 (Test Coverage)

目前的 E2E 测试套件已形成深度的网络化覆盖，保障了核心架构的生产级可用性。

### 1. 业务与授权模式维度
| 维度 | 已验证能力 | 验证说明 |
| :--- | :--- | :--- |
| **自建应用 (SelfBuilt)** | 基础链路、白名单拦截 | 涵盖单机/分布式下的 Token 获取与 API 代理透传。 |
| **代开发应用 (StoreApp)** | 生命周期闭环、灾备恢复、多租户隔离 | 验证了从 AppTicket 接收到 `TEMP_AUTH_CODE` 置换，再到多租户（数十个 OrgID）并行换票、`x-org-id` 强校验 API 代理寻址，以及令牌丢失后的**永久授权码 (Fire Seed) 自动降级恢复**。 |
| **三方授权 (OAuth2)** | 授权码模式、自动续约 | **(架构级约束)** 不支持且已在代码级拦截任何形式的分布式/集群化部署（仅限 Sidecar 或桌面级单机使用）。 |

### 2. 存储与分布式架构维度
| 维度 | 已验证能力 | 验证说明 |
| :--- | :--- | :--- |
| **单机架构 (Local)** | 文件系统读写、降级迁移 | 验证了默认 `innerdb` 的可靠性以及禁止从 SQL 降级回 File 的保护策略。 |
| **分布式同步 (SQL/Redis)**| 共享存储、状态隔离 | 验证了多节点挂载统一 SQLite 或 Redis 时，凭据和缓存的同步一致性。 |
| **高可用与韧性 (HA)** | 故障恢复、断线重连 | 验证了 Redis 宕机重启后的平滑恢复 (Case 18)、Daemon 崩溃自动重启、WebSocket 断线自愈等极端情况。 |

---

## 📝 核心测试用例清单 (Selected Cases)

| ID | 名称 | 验证重点 |
| :--- | :--- | :--- |
| `case_01-03`| 基础授权握手 | 三种核心 App Mode 的标准生命周期启动。 |
| `case_09` | 异步重试 (DLQ) | 验证 Webhook 转发失败后的指数退避重试入库。 |
| `case_13` | 分布式负载均衡 | 多节点场景下的 Webhook 负载均衡接收。 |
| `case_14-15`| SQL 分布式协同 | 验证多节点下的应用票据与 Token 共享竞争。 |
| `case_17-18`| Redis 高可用 | 验证 Redis 后端下的 Token 同步与宕机自适应恢复。 |
| `case_19-20`| 自动化保活 | 验证 AppTicket 缺失主动重发与 OAuth2 Refresh 自动续期。 |
| `case_21` | 零信任安全拦截 | 验证 CLI 对非白名单接口的本地主动拦截防线。 |
| `case_22` | 死信手动运维 | 验证管理员执行 `cowen dlq retry` 的全链路闭环，消除 DLQ 介入盲区。 |
| `case_26` | 架构盲区监控: 并发幂等 | 明确验证分布式高并发下，基于 `msgId` 的防重穿透现状（当前预期为失败或依赖上层保证）。 |
| `case_27` | 架构盲区监控: 混合漂移 | 明确验证 Hybrid Store 在缓存未过期时的底层 SQL 被篡改后的穿透现状。 |
| `case_28` | **多租户高并发隔离**| 验证 StoreApp 模式下单实例支撑海量企业的授权码并发置换，以及 Proxy 基于 `x-org-id` 的精准 `openToken` 动态寻址注入。 |

---

## 🔍 已知的架构边界 (Architectural Boundaries)

目前的测试用例已全面覆盖原本的“测试盲区”，包括通过 `case_26` 和 `case_27` 对分布式并发幂等性与混合存储数据漂移进行了**沙盒断言化监控**。这些不再是未经验证的“盲区”，而是通过 E2E 脚本确认的系统当前的**架构级事实边界**：

1. **并发幂等性依赖于上层 (Case 26 verified)**:
   - 现状：CLI 在 `Broadcast` 模式多节点部署时，多个 Node 收到同一事件会同时触发转发。
   - 边界：系统并未在底层存储引入分布式重锁 (`SETNX` / 唯一索引) 拦截并发。完全依赖平台的 `Load Balancing` 模式或下游业务 Sink 自身的幂等表。
2. **混合存储的 Cache-Aside 现状 (Case 27 verified)**:
   - 现状：当 Redis 缓存为 Warm 状态且 SQL 凭据被非法篡改时，Proxy 会继续使用缓存数据直至其自然过期，发生“数据漂移”。
   - 边界：CLI 严格遵循 Cache-Aside 协议，依赖 Redis 的 TTL 强过期机制，并未引入重资源消耗的后台周期性对账自愈协程。

---

## 🛠️ 测试架构说明

1. **Mock Server** (`tests/mock_server.py`):
   - 模拟畅捷通开放平台的核心 API (Auth, Ticket, OpenAPI)。
   - 提供基于 `x-org-id` 和 `appKey` 的多租户认证验证环境。
   - 提供控制平面 (`/control/...`) 用于向 CLI 发起并发 WebSocket 广播 (模拟平台推送) 或动态篡改服务端行为 (模拟 Token 过期)。

2. **Common Utilities** (`tests/common.sh`):
   - 提供沙盒环境搭建、Daemon 进程守护与追踪、端口冲突检测等通用支持。