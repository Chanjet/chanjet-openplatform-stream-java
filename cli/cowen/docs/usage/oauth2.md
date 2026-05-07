# OAuth2 模式使用指南 (标准工具模式)

OAuth2 模式（也称为“标准工具模式”）是专门为个人开发者和本地调试场景设计的。它通过浏览器交互授权，为 `cowen` 提供最简单的零配置启动体验。

## 🎯 目标场景
- **快速调试**：无需配置 AppSecret 或证书，即可快速调用 OpenAPI。
- **本地工具集成**：通过本地代理为 Postman 或其他脚本提供自动化的身份注入。
- **轻量维护**：自动托管 Token 生命周期，无需关心令牌续约细节。

## 🚀 核心工作流

### 1. 极简初始化
在 OAuth2 模式下，初始化不需要任何应用密钥参数：
```bash
cowen init --profile dev --app-mode oauth2
```
运行后，系统会自动打开您的默认浏览器，引导您完成畅捷通账号的授权登录。

### 2. 状态确认
授权成功后，运行以下命令查看状态：
```bash
cowen status
```
- **Auth Status**: 应显示 `ACTIVE` 并标注当前的 AccessToken。
- **Daemon**: 建议保持运行状态，以实现令牌自动续约。

### 3. 关于守护进程 (Daemon)
OAuth2 模式在 `init` 成功后会尝试自动拉起守护进程。

- **ACTIVE (已启动)**：守护进程会在 AccessToken 过期前（通常 2 小时）利用 RefreshToken 自动进行静默续约，实现“零感知”调用。
- **OFFLINE (未启动)**：如果守护进程未运行，当令牌过期时，`cowen` 会尝试在您执行命令时进行“被动刷新”。这会产生约 1-2 秒的网络延迟。如果 RefreshToken 也已过期（通常 7 天），则需要重新运行 `auth login`。

### 4. 本地代理 (Proxy) 使用指南
这是该模式下最核心的功能之一，允许外部工具直接通过 `cowen` 调用云端 API。

1. **启动并开启代理**：
   ```bash
   # 方式 A：临时开启（命令行指定）
   cowen daemon start --enable-proxy

   # 方式 B：持久化开启（推荐）
   cowen config --proxy-enabled true
   cowen daemon start
   ```
2. **检查端口状态**：运行 `cowen status`。系统会自动分配一个空闲端口，请以 `status` 命令输出的实际端口为准。
   > **提示：如何手动指定端口？**
   > - **初始化时指定**：`cowen init --app-mode oauth2 --proxy-port 9090`
   > - **后期修改配置**：`cowen config --proxy-port 9090`

3. **发起调用**：
   ```bash
   # 核心优势：可省略以下 Header，由 cowen 自动补全
   # 1. openToken: <TOKEN> (身份令牌)
   # 2. appKey: <APP_KEY> (应用标识)
   curl http://127.0.0.1:8000/v1/user/info
   ```
   > [!IMPORTANT]
   > **不可省略的 Header**：
   > 代理仅处理身份鉴权。标准的 HTTP 业务 Header（如 POST 请求时的 `-H "Content-Type: application/json"`）仍需由您根据接口要求自行提供。

## ⚠️ 能力边界

为了保持轻量和易用性，OAuth2 模式在功能上有明确的边界限制：

| 特性 | 支持状态 | 备注 |
| :--- | :--- | :--- |
| **身份类型** | 标准 OAuth2 | 适用于个人/单组织调试 |
| **本地代理 (Proxy)** | ✅ 全力支持 | 提供全自动的 `openToken` 和 `appKey` 注入 |
| **令牌自动续约** | ✅ 全力支持 | 通过守护进程实现 7x24 小时令牌可用 |
| **消息推送 (Webhook)** | ❌ **不支持** | 该模式不接收或转发业务消息推送 |
| **死信管理 (DLQ)** | ❌ **不支持** | 无消息接收能力，故不提供运维指令 |
| **安全性** | ✅ 极高 | 基于 PKCE 协议，本地存储仅限 RefreshToken |

## 🔐 存储限制与架构 (Storage Constraints)

> [!WARNING]
> **分布式部署限制**：由于 OAuth2 模式依赖本地浏览器交互（PKCE 流程）以及特定的本地回调地址处理，该模式 **仅支持本地存储**（`innerdb` 或 `local`）。

### 1. 强制本地存储
OAuth2 模式定位于“本地开发调试工具”。如果您尝试在全局配置为远程数据库（如 MySQL, Postgres, Redis）的环境下使用 OAuth2，系统将由于无法同步授权上下文而报错。

- **支持的存储**: `innerdb` (默认 SQLite), `local` (加密文件)
- **不支持的存储**: `mysql`, `postgres`, `mssql`, `redis`

### 2. 为什么不支持远程存储？
1. **交互性**：OAuth2 流程需要 `cowen` 在本地启动一个临时的 HTTP 监听器来接收授权码（Code）。
2. **安全性**：PKCE 协议要求的 `code_verifier` 是临时生成的，且必须在同一个进程中闭环，难以通过远程数据库跨网络实例共享授权中间态。
3. **定位**：OAuth2 旨在提供“零配置”的开发体验，而非高可用的生产级侧车。

### 3. 集群环境的替代方案
如果您需要在 K8s 集群或分布式生产环境中使用 `cowen` 实现 AccessToken 共享，请根据应用类型选择：
- **[Self-Built (自建模式)](self_built.md)**: 通过 AppKey/AppSecret/Certificate 实现完全自动化的分布式部署。
- **[Store-App (商店应用模式)](store_app.md)**: 支持大规模多租户场景。


---

> [!TIP]
> 如果您的业务场景需要分布式集群部署或接收实时消息推送，请改用 **[Self-Built (自建模式)](self_built.md)** 或 **[Store-App (商店应用模式)](store_app.md)**。
