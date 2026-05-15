# Cowen CLI E2E 测试套件指南

本目录包含了 Cowen CLI 的端到端 (E2E) 自动化测试脚本，用于验证核心功能在不同授权模式、存储模式及分布式环境下的稳定性与健壮性。

## 🚀 如何运行

### 环境准备
- **Python 3**: 用于运行 Mock Server (`tests/infra/mock_server.py`)。
- **SQLite3**: 验证数据库状态。
- **Cargo**: 用于编译待测二进制文件。
- **Redis**: 运行 `case_17` 和 `case_18` 需要本地 Redis 服务。
- **数据库 (MySQL/PostgreSQL)**:
    - **方法 A (推荐 - 稳定)**: 通过 Homebrew 在本地安装并运行。
      ```bash
      make brew-deps-install # 安装依赖
      make local-db-up       # 启动服务
      ```
    - **方法 B (Podman/Docker)**: 运行 `case_31` 和 `case_32` 需要容器环境。
      ```bash
      make db-up             # 启动容器 (自动处理 podman machine)
      ```

### 执行测试
运行所有测试用例：
```bash
# 1. 启动数据库 (推荐使用本地 brew 模式)
make local-db-up

# 2. 运行测试
./tests/runners/run_parallel.sh

# 3. 清理环境
make local-db-down
```

运行单个测试用例（例如 Case 27）：
```bash
cargo build && bash tests/e2e/scripts/case_27_store_app_multi_org_stress.sh
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
| **分布式同步 (SQL/Redis)**| 共享存储、状态隔离 | 验证了多节点挂载统一 SQLite, MySQL, PostgreSQL 或 Redis 时，凭据和缓存的同步一致性。 |
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
| `case_31` | MySQL 共享存储 | 验证 MySQL 后端下的 Token 同步与多节点协同。 |
| `case_32` | PostgreSQL 共享存储 | 验证 PostgreSQL 后端下的 Token 同步与多节点协同。 |
| `case_19-20`| 自动化保活 | 验证 AppTicket 缺失主动重发与 OAuth2 Refresh 自动续期。 |
| `case_21` | 零信任安全拦截 | 验证 CLI 对非白名单接口的本地主动拦截防线。 |
| `case_22` | 死信手动运维 | 验证管理员执行 `cowen dlq retry` 的全链路闭环，消除 DLQ 介入盲区。 |
| `case_23` | **智能自动补全** | 验证 Bash/Zsh 环境下的多级命令与 Profile 自动补全能力。 |
| `case_24` | **全局状态诊断** | 验证 `status --all` 对全量 Profile 的健康度扫描与错误自动归集。 |
| `case_25` | 架构盲区监控: 并发幂等 | 明确验证分布式高并发下，基于 `msgId` 的防重穿透现状（当前预期为失败或依赖上层保证）。 |
| `case_26` | 架构盲区监控: 混合漂移 | 明确验证 Hybrid Store 在缓存未过期时的底层 SQL 被篡改后的穿透现状。 |
| `case_27` | **多租户高并发隔离**| 验证 StoreApp 模式下单实例支撑海量企业的授权码并发置换，以及 Proxy 基于 `x-org-id` 的精准 `openToken` 动态寻址注入。 |
| `case_41` | **认证自愈全链路** | 验证 `logout` 后 `login` 的自动 Fallback 机制。 |

---

## 🔍 已知的架构边界 (Architectural Boundaries)

目前的测试用例已全面覆盖原本的“测试盲区”，包括通过 `case_25` 和 `case_26` 对分布式并发幂等性与混合存储数据漂移进行了**沙盒断言化监控**。这些不再是未经验证的“盲区”，而是通过 E2E 脚本确认的系统当前的**架构级事实边界**：

1. **并发幂等性依赖于上层 (Case 25 verified)**:
   - 现状：CLI 在 `Broadcast` 模式多节点部署时，多个 Node 收到同一事件会同时触发转发。
   - 边界：系统并未在底层存储引入分布式重锁 (`SETNX` / 唯一索引) 拦截并发。完全依赖平台的 `Load Balancing` 模式或下游业务 Sink 自身的幂等表。
2. **混合存储的 Cache-Aside 现状 (Case 26 verified)**:
   - 现状：当 Redis 缓存为 Warm 状态且 SQL凭据被非法篡改时，Proxy 会继续使用缓存数据直至其自然过期，发生“数据漂移”。
   - 边界：CLI 严格遵循 Cache-Aside 协议，依赖 Redis 的 TTL 强过期机制，并未引入重资源消耗的后台周期性对账自愈协程。

---

## 🚀 下一阶段测试演进建议 (Next-Stage Roadmap)

根据最新的代码覆盖率报告（LLVM-COV: 15.19%），虽然核心链路已闭环，但仍存在以下**业务死角 (Business Dead Zones)** 需要在后续版本中攻克：

### 1. 多数据库驱动兼容性 (Database Diversity)
*   **死角**: 当前 E2E 已覆盖 SQLite/MySQL/Postgres。但 MSSQL 驱动代码覆盖率为 0。
*   **建议**: 引入 Dockerized MSSQL 环境，补充 `Case 33` (如果已补齐) 以验证不同数据库方言下的凭据持久化稳定性。

### 2. 极端网络与协议容错 (Network & Protocol Resilience)
*   **死角**: `src/auth/client.rs` 中的超时、断网重试、502/503 错误处理分支未被充分触发。
*   **建议**: 增强 `mock_server.py`，支持模拟网络延迟、丢包及非 JSON 异常响应，验证 Proxy 层的健壮性。

### 3. 运维与诊断指令闭环 (Ops & Diagnostics)
*   **死角**: `cowen service`, `log view`, `config reset` 等运维类指令在自动化脚本中覆盖不足。
*   **建议**: 补充针对 OS 服务管理器（systemd/launchd）的静态配置校验测试。

### 4. 遥测与 AI 引擎链路 (Telemetry & AI Search)
*   **死角**: 由于测试环境通常禁用遥测和 AI，导致 `src/core/telemetry.rs` 和 `src/core/search.rs` 成为大片盲区。
*   **建议**: 在专用的“冒烟测试”中开启相关功能，验证其对系统性能的影响及数据上报的正确性。

---

## 🛠️ 测试架构说明

1. **Mock Server** (`tests/infra/mock_server.py`):
   - 模拟畅捷通开放平台的核心 API (Auth, Ticket, OpenAPI)。
   - 提供基于 `x-org-id` 和 `appKey` 的多租户认证验证环境。
   - 提供控制平面 (`/control/...`) 用于向 CLI 发起并发 WebSocket 广播 (模拟平台推送) 或动态篡改服务端行为 (模拟 Token 过期)。

2. **Common Utilities** (`tests/e2e/scripts/common.sh`):
   - 提供沙盒环境搭建、Daemon 进程守护与追踪、端口冲突检测等通用支持。
