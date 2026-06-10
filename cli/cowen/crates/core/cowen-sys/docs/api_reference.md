# cowen-sys API Reference



本文档动态提炼了 `cowen-sys` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

*(暂无对外暴露的 Trait)*\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct LinuxFingerprint`

- `pub struct LinuxServiceManager`

- `pub struct MacFingerprint`

- `pub struct MacServiceManager`

- `pub struct PluginLoader`

- `pub struct RpcPluginClient`

- `pub struct UnixIpcBinder`

- `pub struct UnixProcessManager`

- `pub struct WinFingerprint`

- `pub struct WinIpcBinder`

- `pub struct WinProcessManager`

- `pub struct WinServiceManager`\n\n## 关键 Enums (状态与枚举)\n*(暂无对外暴露的 Enum)*\n
