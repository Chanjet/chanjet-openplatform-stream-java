# cowen-auth API Reference



本文档动态提炼了 `cowen-auth` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

- `pub trait AuthProvider`

- `pub trait Client`

- `pub trait HttpSender`

- `pub trait TokenPool`\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct AuthClient`

- `pub struct AuthClientBuilder`

- `pub struct AuthProviderValidator`

- `pub struct AuthSessionManager`

- `pub struct CallbackResult`

- `pub struct CredentialsCheck`

- `pub struct InitParams`

- `pub struct MockHttpSender`

- `pub struct OAuth2CallbackListener`

- `pub struct OAuth2Provider`

- `pub struct OAuth2TokenPair`

- `pub struct Pkce`

- `pub struct RequestDecorator`

- `pub struct ReqwestSender`

- `pub struct SelfBuiltProvider`

- `pub struct SimpleResponse`

- `pub struct StoreAppProvider`

- `pub struct StoreAppTokenResponse`

- `pub struct VaultTokenPool`\n\n## 关键 Enums (状态与枚举)\n- `pub enum PlatformEvent`

- `pub enum ProxyRequestAction`

- `pub enum StoreAppTemplate`\n
