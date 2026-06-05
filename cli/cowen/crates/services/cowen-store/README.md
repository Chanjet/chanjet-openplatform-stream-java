# Cowen Store 存储引擎

`cowen-store` 是 Cowen CLI 的核心持久化层，负责处理配置、密钥、Token 票据、审计日志以及死信队列（DLQ）的存储。它支持多种存储驱动，以满足从单机轻量级到大规模分布式集群的不同需求。

## 存储模式详细清单

下表列出了不同模式下数据的具体存储位置和结构：

### 1. 本地文件模式 (`store: local`)
这是最简单的存储方式，所有数据经过加密后序列化存入单个文件中。

*   **存储位置**: `$COWEN_HOME/.seal` (默认)
*   **存储内容**:
    *   所有 Profiles 的配置信息
    *   加密后的 Secrets
    *   OAuth2 Access/Refresh Tokens
    *   App Tickets 与 App Access Tokens

---

### 2. 本地数据库模式 (`store: innerdb` 或 `sqlite`)
基于 SQLite 的嵌入式数据库存储，支持复杂的查询和审计日志。

*   **存储位置**: `$COWEN_HOME/cowen.db` (默认，可通过 `db_url` 自定义路径)
*   **逻辑说明**:
    *   `innerdb` 是 `sqlite` 的语义化别名。在代码实现上，两者最终都会映射为 SQLite 协议并进入 `SqlStore` 驱动。
    *   **缓存行为**: 若配置了混合模式（Hybrid），两种模式在缓存读取、回填及失效逻辑上**完全一致**。
    *   **区别**: `innerdb` 侧重于开箱即用的本地体验，自动定位到应用主目录；`sqlite` 侧重于通过 URL 灵活配置数据库参数。
*   **表结构清单**:
    | 表名 | 说明 | 核心字段 |
    | :--- | :--- | :--- |
    | `cowen_config` | 常规配置项 | `profile`, `item_key`, `item_value`, `version` |
    | `cowen_secret` | 敏感密钥 (加密存储) | `profile`, `item_key`, `item_value` |
    | `cowen_token` | 临时性令牌 | `profile`, `item_key`, `item_value`, `expires_at` |
    | `cowen_app_token` | 自建应用/工具 AccessToken | `app_key`, `token_value`, `expires_at` |
    | `cowen_tenant_token` | 租户级令牌 (Access/Refresh) | `profile`, `token_type`, `token_value` |
    | `cowen_ticket` | 应用票据 (SuiteTicket) | `app_key`, `ticket_value` |
    | `cowen_permanent_code` | 永久授权码 (永久 Code) | `app_key`, `org_id`, `code_value` |
    | `cowen_audit` | 行为审计日志 | `id`, `profile`, `level`, `message`, `fields` |
    | `cowen_dlq` | 死信队列 (消息积压) | `id`, `profile`, `topic`, `payload`, `error` |
+
+> [!IMPORTANT]
+> **职责边界**:
+> *   在 **Hybrid (混合模式)** 下，本地数据库作为 **持久层** 依然负责所有数据的落盘存储。
+> *   在 **Redis 模式** 下，本地数据库模式被 **完全跳过**，不负责任何存储。

---

### 3. 分布式数据库模式 (`store: mysql` 或 `postgres`)
适用于多节点部署，确保所有节点共享同一份配置和令牌状态。

*   **存储位置**: 远程数据库实例
*   **存储结构**: 与上述 SQLite 表结构完全一致。

---

### 4. Redis 模式 (`store: redis`)
**高性能全委派存储方案**。

在这种模式下，Cowen 将所有的存储职责**完全委派**给 Redis。此时，本地的 `innerdb`、`sqlite` 或外部 SQL 数据库**完全不参与工作**，也不会在磁盘上创建任何数据库文件。

*   **存储位置**: Redis 实例
*   **Key 命名规范**: `{profile}:{prefix}:{key}`
*   **数据结构清单**:
    | Key 模式 | Redis 类型 | 存储内容说明 |
    | :--- | :--- | :--- |
    | `{profile}:cfg:{key}` | STRING | 配置项 |
    | `{profile}:sec:{key}` | STRING | 敏感密钥 (加密) |
    | `{profile}:tok:access` | STRING | 租户 AccessToken (JSON 格式) |
    | `{profile}:tok:refresh` | STRING | 租户 RefreshToken (JSON 格式) |
    | `app:{app_key}:tok_v2:app_access`| STRING | 应用级 AccessToken |
    | `app:{app_key}:tic:v1` | STRING | 应用票据 SuiteTicket |
    | `app:{app_key}:opc:{org_id}` | STRING | 机构永久码 |
    | `{profile}:audit:log` | LIST | 审计日志 (LPUSH/LTRIM 队列) |
    | `{profile}:dlq:{topic}` | LIST | 死信消息队列 (RPUSH/LPOP) |
    | `{profile}:__keys__` | SET | 已存在配置键的索引清单 |

---

### 5. 混合模式 (`store: hybrid`)
**推荐的高可用生产方案**。

混合模式通过结合 SQL 数据库的持久化能力和 Redis 的内存级响应速度，实现了真正意义上的高可用存储方案。

#### 核心协作机制
*   **分层存储**:
    *   **持久层 (Persistence)**: 使用 MySQL/PostgreSQL/SQLite。存储所有配置、历史审计日志和死信队列，确保数据不丢失。
    *   **同步层 (Sync Layer/Cache)**: 使用 Redis。存储所有 Profiles 的 AccessToken、App Ticket 等高频变动数据，确保集群内各节点状态秒级同步。
*   **读策略 (Read-Through)**:
    1. 首先尝试从 Redis 读取。
    2. 若 Redis 缺失，则回源从 SQL 数据库读取。
    3. 读取成功后，自动将数据回写（Populate）到 Redis，供后续使用。
*   **写策略 (Write-Through)**:
    1. 数据首先写入 SQL 数据库，确保事务完整性和持久化。
    2. 写入成功后，同步更新或使 Redis 中的对应缓存失效。
*   **一致性保证**:
    *   对于 **条件写 (CAS)** 操作，系统在成功更新 SQL 后会立即**删除** Redis 中的旧条目（Invalidation），强制后续读取操作执行回源查询，彻底杜绝多节点环境下的“脏缓存”问题。

#### 字段级取值逻辑详细清单

在混合模式下，不同类型的数据遵循不同的路由与同步策略：

| 数据类型 | 涉及字段 | 取值逻辑 (Getter) | 持久化逻辑 (Setter) |
| :--- | :--- | :--- | :--- |
| **高频令牌类** | AccessToken, RefreshToken, AppTicket, PermanentCode | **Read-Through**: Redis 优先，缺失则回源 SQL 并回填 | **Write-Through**: 先写 SQL，成功后同步更新 Redis |
| **常规配置类** | Config Items | **Read-Through**: 同上 | **Write-Through**: 先写 SQL，成功后更新 Redis |
| **敏感密钥类** | Secrets | **Persistence Only**: 仅从 SQL 读取 | **Persistence Only**: 仅写入 SQL (不入 Redis) |
| **原子更新类** | CAS (Conditional Set) | **Read-Through**: 同上 | **Write-And-Invalidate**: 先写 SQL，成功后**删除** Redis 缓存 |
| **审计日志** | Audit Logs | **Persistence Only**: 仅从 SQL 读取 (确保完整性) | **Dual-Write**: 写入 SQL 持久化，同时 LPUSH 到 Redis 滚动队列 |
| **可靠消息** | DLQ (死信队列) | **Persistence Only**: 仅通过 SQL 操作 | **Persistence Only**: 仅写入 SQL (确保持久化) |

> [!TIP]
> **关于 Secret 的设计**: 为了最大化安全性，混合模式默认不对 `get_secret` 进行 Redis 缓存，以防止敏感信息在内存中多点散落。

#### 为什么这是高可用首选？
在典型的容器化部署中，Pod 可能会频繁重启。混合模式允许新启动的节点立即通过 Redis 获取最新的全局令牌状态，同时通过 SQL 保持完整的历史记录和配置备份，兼顾了性能与数据安全。

## 驱动选择建议

| 场景 | 推荐模式 | 理由 |
| :--- | :--- | :--- |
| **个人开发/本地调试** | `local` 或 `innerdb` | 无需外部依赖，开箱即用 |
| **单机生产环境** | `innerdb` | 性能优秀且支持审计日志 |
| **多机集群/高可用** | `hybrid` | 确保 Token 在集群间实时同步，防止刷新冲突 |
| **极高性能需求** | `redis` | 纯内存操作，响应速度最快 |

## 配置示例 (`app.yaml`)

```yaml
storage:
  store: innerdb
  db_url: "sqlite:///path/to/your/custom.db"
```

或分布式 Redis:

```yaml
storage:
  store: redis
  db_url: "redis://:password@127.0.0.1:6379/0"
```
