# Cowen Auth

畅捷通 Cowen CLI 的核心身份认证与生命周期管理组件。

## 🎯 职责 (Responsibility)
- **协议握手 (Auth Handshake)**: 负责 AppTicket, AccessToken, RefreshToken 的完整获取与自动续期流程。
- **多模式适配 (Multi-Mode)**: 支持自建应用 (Self-Built)、OAuth2 应用及应用商店应用 (Store-App) 三种认证模式。
- **令牌池管理 (Token Pooling)**: 基于 `Vault` 的并发安全令牌缓存与预取。
- **请求装饰 (Request Decoration)**: 自动化执行请求签名、Timestamp 注入与鉴权头填充。

## 🛠️ 核心能力 (Capabilities)
- **AuthProvider SPI**: 插件化支持不同认证协议。
- **VaultTokenPool**: 跨进程安全的令牌生命周期自动机。
- **AuthClient**: 提供统一的鉴权接口封装。
- **Diagnostics**: 提供针对凭据状态、时钟偏差及网络联通性的深度诊断能力。

## 📦 外部依赖 (Key Dependencies)
- `cowen-common`: 核心模型与配置依赖。
- `reqwest`: 执行鉴权握手请求。

## ⚠️ 注意事项 (Constraints)
- **存储无关性**: 严禁直接操作文件系统或数据库，必须通过 `Vault` Trait 进行数据持久化。
- **协议边界**: 本模块仅关注“如何获取令牌”，不关注“如何启动后台守护进程”（后者由 `cowen-server` 负责）。
