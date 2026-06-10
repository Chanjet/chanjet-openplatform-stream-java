# cowen-common API Reference



本文档动态提炼了 `cowen-common` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

- `pub trait AsStatusUI`

- `pub trait AuditDomain`

- `pub trait CacheBuilder`

- `pub trait ConfigDomain`

- `pub trait DaemonService`

- `pub trait DlqDomain`

- `pub trait ManagementDomain`

- `pub trait PermanentCodeDomain`

- `pub trait ResetTask`

- `pub trait SecretDomain`

- `pub trait SessionDomain`

- `pub trait StatusCollector`

- `pub trait Store`

- `pub trait StoreBuilder`

- `pub trait StoreItem`

- `pub trait TicketDomain`

- `pub trait TokenDomain`

- `pub trait Vault`\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct ApiResponseDto`

- `pub struct AppConfig`

- `pub struct AuditEntry`

- `pub struct AuthInterceptor`

- `pub struct AuthProgressInfo`

- `pub struct AuthSession`

- `pub struct CacheBuilderRegistration`

- `pub struct Config`

- `pub struct DaemonClient`

- `pub struct DaemonInfo`

- `pub struct DaemonStatus`

- `pub struct DlqMessage`

- `pub struct DummyDaemonService`

- `pub struct EventBus`

- `pub struct FinalizeRequest`

- `pub struct IpcClaims`

- `pub struct Item`

- `pub struct LogConfig`

- `pub struct MonitorClient`

- `pub struct OAuth2TokenPair`

- `pub struct PluginManifest`

- `pub struct ProgressQuery`

- `pub struct ResetEngine`

- `pub struct SecurityConfig`

- `pub struct StatusContext`

- `pub struct StatusEntry`

- `pub struct StorageConfig`

- `pub struct StoreBuilderRegistration`

- `pub struct TelemetryEvent`

- `pub struct Ticket`

- `pub struct Token`

- `pub struct TokenIdentity`

- `pub struct WasmInterceptorContribution`\n\n## 关键 Enums (状态与枚举)\n- `pub enum AuthMode`

- `pub enum AuthStatus`

- `pub enum CommonTemplate`

- `pub enum CowenError`

- `pub enum DaemonResponse`

- `pub enum GlobalEvent`

- `pub enum IpcRole`

- `pub enum SecurityLevel`

- `pub enum StatusLevel`

- `pub enum WorkerStateDto`\n
