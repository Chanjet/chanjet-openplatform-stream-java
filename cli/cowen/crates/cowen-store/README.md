# Cowen Store

畅捷通 Cowen CLI 的统一持久化与存储治理组件。

## 🎯 职责 (Responsibility)
- **多引擎支持 (Multi-Engine)**: 封装不同类型的数据库驱动，并抹平其方言差异。
- **存储 SPI 实现**: 响应 `cowen-common` 定义的 `Store` 与 `Vault` 契约。
- **数据一致性 (CAS)**: 提供基于版本号的 Compare-And-Swap 原子更新能力。
- **存储治理 (Migration)**: 负责不同存储后端之间的数据全量迁移与克隆。

## 🛠️ 核心能力 (Capabilities)
- **FileStore**: 基于本地加密文件的零依赖存储，适用于轻量级开发环境。
- **SqlStore**: 统一支持 SQLite, MySQL, PostgreSQL, MSSQL。
- **RedisStore**: 支持分布式环境下的高性能 Key-Value 存储与共享。
- **HybridStore**: 自动化的装饰器模式实现，支持“内存/Redis + 数据库”的多级缓存架构。
- **StoreMigrator**: 安全的跨引擎数据迁移引擎。

## 📦 外部依赖 (Key Dependencies)
- `sqlx`: 异步 SQL 驱动框架。
- `redis`: Redis 客户端。
- `inventory`: 实现存储引擎的自动发现与注册（SPI 模式）。

## 🚦 使用说明 (Usage)
通过 URL Schema 自动构建存储实例：
```rust
let store = cowen_store::create_store_from_url("redis://127.0.0.1:6379", app_dir, fingerprint).await?;
```

## ⚠️ 注意事项 (Constraints)
- **无状态逻辑**: 本 crate 仅负责数据的存取，严禁包含任何业务鉴权或 OpenAPI 校验逻辑。
- **安全隔离**: 敏感数据（Secrets）的存取必须经过 `Vault` 封装，禁止直接明文暴露在非受控存储中。
