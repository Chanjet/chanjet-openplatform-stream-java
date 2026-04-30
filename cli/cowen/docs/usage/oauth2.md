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
cowen init --profile dev --mode oauth2
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
   > - **初始化时指定**：`cowen init --mode oauth2 --proxy-port 9090`
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

## 🔐 全局存储与缓存架构 (Storage & Cache)

虽然 OAuth2 模式常用于本地调试，但其底层依然受全局存储策略控制，支持多机协同开发。

### 1. 配置选项
- **Store (存储)**: `innerdb` (默认), `mysql`, `postgres`, `mssql`, `redis`。
- **Cache (加速)**: `none`, `redis` (Hybrid 模式)。

### 2. 五大配置场景

| 场景 | 存储 (Store) | 缓存 (Cache) | 适用阶段 |
| :--- | :--- | :--- | :--- |
| **A (默认)** | `innerdb` (SQLite) | `memory` | 个人开发 / 轻量级授权使用 |
| **B (集群化)** | `mysql` / `postgres` / `mssql` | `redis` | 企业级多用户授权共享 |
| **C (生产级全家桶)** | `mysql` / `postgres` / `mssql` | `redis` | 集群推荐：配置远程 DB 与 Redis 实现多机同步 |
| **D (纯云端运行)** | `redis` | `none` | 云原生推荐：直接将 Redis 设为主存储，本地零文件 |
| **E (极简兼容模式)** | `local` | `none` | Legacy：仅用于老版本数据文件兼容 |

---

## 🔄 数据迁移 (Migration)

如果您需要将本地调试的身份信息搬迁到云端数据库，可以使用以下指令：

```bash
# 示例：从本地搬迁到生产环境 MySQL
cowen store migrate --to "mysql://user:pass@host:3306/db"
```

> [!TIP]
> 运行 `cowen store status` 可随时检查当前架构的连接健康状况。

> [!TIP]
> 如果您的业务场景需要接收实时消息推送（如订单提醒），请改用 **[Self-Built (自建模式)](self_built.md)**。
