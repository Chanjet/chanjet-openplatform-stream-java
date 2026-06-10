# cowen-capabilities API Reference



本文档动态提炼了 `cowen-capabilities` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

- `pub trait NativeApiRegistryCapability`
  > **语义推演**：系统核心网关路由表抽象。模型推演：在运行时接管远端 OpenAPI 规约与本地权限列表的双向同步，决定某个 Request 能否被放行，是安全策略落地的第一站。
- `pub trait NativeAuditCapability`
  > **语义推演**：全域合规审计抽象。模型推演：负责异步将涉及数据篡改的请求上下文（Request/Response、耗时、租户信息）进行结构化脱敏转储，满足不可抵赖性。
- `pub trait NativeAuthCapability`
  > **语义推演**：统一身份认证总线。模型推演：隔离了具体的 Token 生命周期逻辑，对外统一提供凭证解析、验签与主动轮换能力，彻底解耦 AuthProvider。
- `pub trait NativeConfigCapability`
  > **语义推演**：全局配置订阅源。模型推演：支持从本地文件和 CLI Flag 甚至远程配置中心实时加载策略并派发变更通知，避免配置穿透导致的全系统重启。
- `pub trait NativeDlqCapability`
  > **语义推演**：金融级死信队列核心。模型推演：当外网通信发生严重抖动、限流被触发，或出现无法恢复的 HTTP 502 时，将完整的 API Payload 落盘保存，以便在网络恢复后由监控总线进行安全回放，保障核心交易不丢单。
- `pub trait NativeSearchCapability`
  > **语义推演**：向量化语义召回引擎抽象。模型推演：提供与底层模型后端（如内嵌 ONNX 引擎）对话的接口，执行特征提取及 KNN 近似最近邻检索。
- `pub trait NativeSystemCapability`
  > **语义推演**：宿主系统交互层。模型推演：封装了进程锁定、PID 存活检查等操作系统级原语，解决多实例启动的冲突检测（互斥锁）。
- `pub trait NativeWorkerCapability`
  > **语义推演**：常驻任务调度抽象。模型推演：统一纳管周期性的心跳保活、DLQ 重试等后台线程，保障它们在进程退出时被优雅关闭 (Graceful Shutdown)。
- `pub trait PublicSystemCapability`\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct CapabilityRegistry`

- `pub struct DLQEntry`

- `pub struct DefaultApiRegistry`
  > **语义推演**：基于内存或本地 SQLite 支撑的路由表默认实现。引入读写锁 (RwLock) 以支持百万级 QPS 并发鉴权。
- `pub struct DefaultAuditCapability`

- `pub struct DefaultAuthCapability`
  > **语义推演**：核心鉴权业务的承载实体。负责驱动多路由 OAuth 校验、签发内部上下文 Token。
- `pub struct DefaultConfigCapability`

- `pub struct DefaultDlq`

- `pub struct DefaultPublicSystem`

- `pub struct DefaultSearch`

- `pub struct DefaultSystem`

- `pub struct DefaultWorkerCapability`

- `pub struct DlqStore`
  > **语义推演**：SQLite 或 Redis 数据源驱动下的死信持久化句柄。负责数据的序列化与反序列化（基于 JSON/BSON），保证落盘的原子性。
- `pub struct DomainApiListRequest`
  > **语义推演**：业务指令模型：API 列表检索请求，通常由 CLI 通过 IPC 传递给 Daemon，要求进行规约缓存穿透。
- `pub struct DomainApiListResponse`

- `pub struct DomainApiSpecRequest`

- `pub struct DomainApiSpecResponse`

- `pub struct DomainCallApiRequest`
  > **语义推演**：业务核心 DTO：跨端 API 代理请求封装。包含标准化的 Header、Query、Body 解析，是 gRPC 与内部核心 Traits 间数据流转的标准化单元。
- `pub struct DomainCallApiResponse`

- `pub struct DomainDlqListRequest`

- `pub struct DomainDlqListResponse`

- `pub struct DomainDlqPurgeRequest`

- `pub struct DomainDlqPurgeResponse`

- `pub struct DomainDlqRetryRequest`
  > **语义推演**：业务指令模型：手工/定时触发死信回放时的参数模型，用于精细控制重试批次与并发度。
- `pub struct DomainDlqRetryResponse`

- `pub struct DomainDlqViewRequest`

- `pub struct DomainDlqViewResponse`

- `pub struct DomainDoctorRequest`

- `pub struct DomainDoctorResponse`

- `pub struct DomainPluginHandshakeRequest`

- `pub struct DomainPluginHandshakeResponse`

- `pub struct DomainStoreStatusRequest`

- `pub struct DomainStoreStatusResponse`

- `pub struct DomainSystemResetRequest`

- `pub struct DomainSystemResetResponse`

- `pub struct DomainSystemStatusRequest`

- `pub struct DomainSystemStatusResponse`

- `pub struct Forwarder`

- `pub struct OpenApiParser`\n\n## 关键 Enums (状态与枚举)\n*(暂无对外暴露的 Enum)*\n
