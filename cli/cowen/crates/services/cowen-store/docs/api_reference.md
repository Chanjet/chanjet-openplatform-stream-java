# cowen-store API Reference



本文档动态提炼了 `cowen-store` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

- `pub trait SchemaMigration`

- `pub trait SqlBuilder`

- `pub trait SqlDriver`\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct FileStore`

- `pub struct HybridStore`

- `pub struct MonolithicSealStore`

- `pub struct MssqlBuilder`

- `pub struct MssqlDriver`

- `pub struct MySqlBuilder`

- `pub struct MySqlDriver`

- `pub struct PostgresBuilder`

- `pub struct PostgresDriver`

- `pub struct RedisStore`
  > **语义推演**：面向 Kubernetes 部署、Serverless 扩展的分布式 KV 存储实现。利用 Lua 脚本保证更新凭证等高并发业务的 CAS 原子性。
- `pub struct SqlBuilderRegistration`

- `pub struct SqlStore`

- `pub struct SqliteBuilder`

- `pub struct SqliteDlqProvider`

- `pub struct SqliteDriver`

- `pub struct StorageCheck`

- `pub struct StorageResetTask`

- `pub struct StoreMigrator`

- `pub struct StoreVault`\n\n## 关键 Enums (状态与枚举)\n- `pub enum MigrationMode`\n
