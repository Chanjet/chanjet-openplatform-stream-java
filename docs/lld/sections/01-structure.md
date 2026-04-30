# 静态依赖视图 (Static Structure)

## 1. 模块依赖防腐图 (Anti-Corruption Layers)

```mermaid
classDiagram
    class Store {
        <<interface>>
        +get(profile, key) String
        +set(profile, key, value)
        +delete(profile, key)
    }

    class FileStore {
        -path PathBuf
        -key [u8; 32]
    }

    class RedisStore {
        -client redis::Client
    }

    class SqlStore {
        -driver SqlDriver
    }

    class SqlDriver {
        <<interface>>
        +get(profile, key) String
        +set(profile, key, value)
        +delete(profile, key)
    }

    class MySqlDriver {
        -pool Pool~MySql~
    }
    class PostgresDriver {
        -pool Pool~Postgres~
    }
    class MssqlDriver {
        -pool Pool~Mssql~
    }

    class HybridStore {
        -cache Store
        -persistence Store
    }

    Store <|.. FileStore
    Store <|.. RedisStore
    Store <|.. SqlStore
    Store <|.. HybridStore
    SqlStore --> SqlDriver
    SqlDriver <|.. MySqlDriver
    SqlDriver <|.. PostgresDriver
    SqlDriver <|.. MssqlDriver

    class VaultService {
        -store Store
        +get_secret(profile, key) String
    }

    class AuthService {
        -store Store
        +refresh_token(profile) Token
    }

    VaultService --> Store
    AuthService --> Store
```

## 2. 核心包划分
- `crate::core::store`: 包含 `Store` Trait 定义及多种后端驱动。
- `crate::core::vault`: 修改为依赖 `Store` Trait，而非硬编码 `MultiVault`。
- `crate::auth`: 依赖 `Store` 进行 Token 的分布式持久化与缓存。

---
*关联 HLD：[模块划分与依赖关系](../../hld/sections/04-modules.md)*
