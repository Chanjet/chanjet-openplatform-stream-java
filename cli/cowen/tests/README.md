# Cowen CLI E2E 测试套件指南

本目录包含了 Cowen CLI 的端到端 (E2E) 自动化测试脚本，用于验证核心功能在不同授权模式、存储模式及分布式环境下的稳定性。

## 🚀 如何运行

### 环境准备
- **Python 3**: 用于运行 Mock Server (`tests/mock_server.py`)。
- **SQLite3**: 验证数据库状态。
- **Cargo**: 用于编译待测二进制文件。

### 执行测试
运行所有测试用例：
```bash
bash tests/run_suites.sh
```

运行单个测试用例（例如 Case 15）：
```bash
cargo build && bash tests/case_15_store_app_shared_storage.sh
```

---

## 📊 测试覆盖率统计

### 1. 应用授权模式 (App Mode) 维度

| 模式 | 已覆盖场景 | **非覆盖场景 (测试盲区)** |
| :--- | :--- | :--- |
| **SelfBuilt** (自建应用) | 基础握手、分布式 Token 同步 (SQLite) | 分布式 Token 同步 (Redis) |
| **StoreApp** (代开发应用) | 基础握手、Webhook Ticket 拦截、分布式同步 (SQLite) | Ticket 过期自动重续、分布式 Token 同步 (Redis) |
| **OAuth2** (三方授权) | 基础 Authorization Code 交换 | **分布式环境禁用**（代码级强制拦截）、刷新令牌自动续约 |

### 2. 存储模式 (Store Mode) 维度

| 模式 | 已覆盖场景 | **非覆盖场景 (测试盲区)** |
| :--- | :--- | :--- |
| **Local** (本地文件) | 绝大部分单机功能测试 (Case 01-12) | 极其海量 (10w+) Key 的加载性能 |
| **SQLite** | 分布式共享存储同步 (Case 14-15) | 并发写入压力测试、数据库连接断开自动重连 |
| **Redis** | 无 | **全部**。目前尚无针对 Redis 存储模式的 E2E 验证用例 |
| **Hybrid** (混合模式) | 无 | **全部**。Local + SQL/Redis 的降级与同步策略尚未通过 E2E 验证 |

---

## 📝 测试用例清单

| ID | 名称 | 验证重点 |
| :--- | :--- | :--- |
| `case_01` | SelfBuilt 基础 | 自建应用模式下的单机握手流程 |
| `case_02` | StoreApp 基础 | 代开发应用模式下的单机握手流程 |
| `case_03` | OAuth2 基础 | OAuth2 授权码交换流程 |
| ... | ... | ... |
| `case_13` | 分布式 LB | 多节点场景下的 Webhook 负载均衡与幂等性 |
| `case_14` | SQLite 分布式 (SelfBuilt) | 自建应用在共享 SQLite 下的 Token 同步 |
| `case_15` | SQLite 分布式 (StoreApp) | 代开发应用在共享 SQLite 下 females Ticket/Token 同步 |

---

## 🛠️ 测试架构说明

1. **Mock Server** (`tests/mock_server.py`):
   - 模拟畅捷通开放平台的所有核心 API (Auth, Ticket, Spec)。
   - 提供 `/control` 接口供测试脚本检查 Mock Server 收到的请求详情（用于断言）。

2. **Common Utilities** (`tests/common.sh`):
   - 包含环境初始化、Daemon 进程管理、日志清理等通用函数。

3. **Shared DB**:
   - 在分布式测试中，多个节点会通过指定 `db_url` 指向同一个 `.db` 文件来模拟真实集群环境。
