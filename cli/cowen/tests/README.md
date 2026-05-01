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

---

# Cowen CLI 下一阶段测试任务计划 (Next-Stage Test Plan)

基于当前 E2E 测试覆盖率分析及最近发现的工程债务，制定以下下一阶段测试任务，旨在消除 Redis 存储、令牌续约、安全拦截及系统弹性等核心盲区。

## 🎯 核心目标
1.  **存储扩展**: 实现并验证 Redis 作为存储/缓存后端的分布式稳定性。
2.  **生命周期完备性**: 覆盖令牌 (Token/Ticket) 过期自动续约的极端场景。
3.  **安全合规性**: 配合生产代码修复，验证 Proxy 层 OpenAPI 白名单拦截。
4.  **弹性与韧性**: 模拟中间件 (DB/Redis/网络) 故障下的降级与恢复能力。

## 📋 任务拆解

### 1. Redis 分布式存储专项 (High Priority)
目前测试脚本仅覆盖了 SQLite 共享存储，Redis 领域仍是空白。
- [ ] **Case 17: Redis 分布式 Token 同步**: 验证多个节点在 Redis 后端下，Token 的争抢、缓存与一致性。
- [ ] **Case 18: Redis 故障恢复**: 模拟 Redis 重启/断连，验证 CLI 是否能自动重连或进入保护模式。

### 2. 令牌与票据自动续约 (Critical Path)
- [ ] **Case 19: StoreApp Ticket 自动重发**: 模拟 AppTicket 推送失败或过期，验证 CLI 是否能主动触发 `/auth/appTicket/resend`。
- [ ] **Case 20: OAuth2 刷新令牌 (Refresh Token) 续约**: 模拟 AccessToken 过期，验证 CLI 是否能在调用 API 前自动使用 Refresh Token 换取新令牌。

### 3. 安全与拦截验证 (Security Debt)
- [ ] **Case 21: OpenAPI 白名单强制校验 (CLI 侧)**: 
  > [!NOTE]
  > 仅适用于 `self-built` (自建应用) 与 `oauth2` (三方授权) 模式。当前白名单拦截逻辑主要在 CLI `api` 命令层实现，Daemon Proxy 层尚未强制拦截。
  - 验证：使用 `cowen api [METHOD] [PATH]` 访问不在白名单的路径，应由 CLI 拦截并提示权限不足。
  - 验证：动态更新 Spec 后，CLI 的拦截行为是否同步。

### 4. 极端场景与弹性测试 (Reliability)
- [ ] **Case 22: 海量配置加载性能**: 在单个 Profile 下写入 10,000 条配置，验证 `config` 命令的响应时间。
- [ ] **Case 23: 混合存储 (Hybrid) 降级测试**: 模拟分布式 SQL 挂掉，验证系统是否能根据策略回退到本地 FileStore 或内存缓存读取关键凭据。

### 5. CLI 辅助功能验证 (Usability)
- [ ] **Case 24: 自动补全 (Shell Completion)**: 验证 `completion --install` 在不同 Shell (Bash/Zsh) 下生成的脚本有效性。
- [ ] **Case 25: 系统健康检查 (Status --all)**: 验证在拥有 50+ Profiles 时，`status --all` 是否能准确汇总各域的异常状态（如过期、连接失败）。

## 🛠️ 技术支撑要求
1.  **环境扩展**: 测试运行环境需安装 `redis-server` (本地或 Docker)。
2.  **Mock 增强**: `mock_server.py` 需要支持可配置的 Token 过期时间及手动模拟网络延迟/故障。
3.  **自动化流水线**: 将 `run_parallel.sh` 集成至 GitLab CI，确保每次提交均进行全量回归。

## 📅 时间排期建议
| 阶段 | 重点任务 | 预期产出 |
| :--- | :--- | :--- |
| **P1** | Redis 存储 + Token 续约 | 核心分布式能力闭环 |
| **P2** | 安全拦截 (403) + 故障恢复 | 生产环境就绪 (Production-Ready) |
| **P3** | 性能压测 + 辅助功能 | 交付级品质保证 |
