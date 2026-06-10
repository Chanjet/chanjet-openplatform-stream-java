# cowen-search API Reference



本文档动态提炼了 `cowen-search` 模块对其他模块暴露的核心公共 API (Public API) 契约。



> 注：以下列表为代码扫描提取出的核心外化对象，详细的函数签名和生命周期请参阅代码库运行 `cargo doc` 输出的内容。



## 核心 Traits (抽象规范)

- `pub trait SearchProvider`\n\n## 核心 Structs (实体结构与服务对象)\n- `pub struct FallbackProvider`

- `pub struct SearchDocument`

- `pub struct SearchProviderFactory`

- `pub struct SidecarSearchProvider`

- `pub struct StringMatchProvider`\n\n## 关键 Enums (状态与枚举)\n*(暂无对外暴露的 Enum)*\n
