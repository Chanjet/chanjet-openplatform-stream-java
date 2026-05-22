# Store App 模式使用指南 (多租户侧车模式)

Store App 模式（应用商店模式）是为 **ISV (独立软件供应商)** 量身定制的高级方案。它作为您的业务系统的“智能侧车 (Sidecar)”，自动托管成千上万个租户的身份交换、令牌刷新以及消息归集。

## 🎯 目标场景
- **SaaS 应用**：ISV 开发的多租户 SaaS，需要集成畅捷通能力。
- **大规模托管**：自动化维护海量不同企业租户的 Token 生命周期。
- **统一网关**：将分散的租户消息统一汇聚并转发给 ISV 服务端。

## 🚀 核心工作流

### 1. 初始化配置 (三要素强制)
初始化商店模式时，您必须提供以下关键凭据：
```bash
cowen init --profile isv-prod \
           --app-mode store-app \
           --app-key <YOUR_APP_KEY> \
           --app-secret <YOUR_APP_SECRET> \
           --encrypt-key <YOUR_ENCRYPT_KEY>
```
> [!IMPORTANT]
> **安全性与存储模式 (Vault)**：
> 您的 `AppSecret` 和 `EncryptKey` 将存储在 **Vault** 中。系统采用 **设备指纹加密 (Machine-Fingerprint Encryption)** 方案，基于主机名、系统版本等硬件信息派生密钥，确保凭据在物理层面被隔离保护。

### 2. 侧车自动就绪
初始化成功后，后台守护进程会**自动启动**。
- **职责**：实时监听并处理平台推送的 `AppTicket`、`TempAuthCode`（用于租户首次授权）以及所有已授权租户的业务消息。
- **持久化**：租户的授权信息（永久码）会自动存储在本地，确保 `cowen` 重启后依然能恢复所有租户的 Token 维护。

### 3. 多租户 API 调用 (通过 Proxy)
由于 CLI 无法在单次指令中仲裁租户上下文，商店模式下禁用了 `cowen api call` 指令。您必须通过本地代理进行调用：

- **调用方法**：
  在请求 Header 中携带租户标识：
  - **`x-org-id`** (必填): 租户企业 ID。
  - **`x-user-id`** (可选): 租户下的具体用户 ID，用于调用需要用户授权上下文的 API。
  ```bash
  # cowen 会根据 x-org-id (及可选的 x-user-id) 自动注入对应的租户 Token
  curl http://127.0.0.1:8080/v1/api/path \
       -H "x-org-id: tenant_12345" \
       -H "x-user-id: user_67890"
  ```
- **自动注入**：代理会自动补全符合规范的 **`openToken`** 和 **`appKey`**。

### 4. 消息推送与 Webhook 转发
`cowen` 会将所有租户的消息汇聚后，转发至您的 ISV 服务端。

- **配置转发地址**：
  ```bash
  cowen config set security.webhook_target http://127.0.0.1:5000/callback --global
  ```
- **多租户识别**：转发时，消息体中通常包含租户信息（如 `org_id`）。您可以解析消息体来识别租户归属。
- **安全约束**：出于 SSRF 防护要求，转发目标仅限本地回环地址 (`127.0.0.1` / `localhost` / `[::1]`)。

## 🔐 租户授权流 (Tenant Authorization Flow)

商店应用模式的核心是管理海量租户的授权。`cowen` 提供了两种方式来回收授权码并将其转换为永久码。

### 1. 引导租户进行 OAuth2 授权
ISV 需要引导企业管理员访问畅捷通开放平台的授权页面。
- **授权 URL 示例**:
  `https://market.chanjet.com/user/v2/authorize?client_id=<YOUR_APP_KEY>&response_type=code&scope=all&state=tenant_123&redirect_uri=https://your-app.com/callback`
- **关键参数说明**:
    - `client_id`: 您的应用 AppKey。
    - `state`: 建议填写租户在您系统中的唯一 ID。
    - `redirect_uri`: 授权后的跳转地址（由您的业务系统接收）。

### 2. 回收授权码 (Auth Code)
授权成功后，ISV 有两种方式完成后续的换票过程：

#### A. 全自动回收 (推荐：基于 Stream Bridge)
如果您的 `cowen` 守护进程已启动（`cowen daemon start`），它会监听开放平台的实时推送：
1. **自动拦截**: 开放平台通过 WebSocket 直接将 `TempAuthCode` 推送给 `cowen`。
2. **自动换票**: `cowen` 拦截消息后，会立即自动将其交换为 **`PermanentAuthCode`** (永久授权码)。
3. **自动归档**: 永久码和初始 Token 会被加密存入您的存储后端（MySQL/Redis），`cowen` 随即开始自动续约。

#### B. 主动交换 (基于 Proxy 代理)
如果您希望在业务系统的 Callback 逻辑中直接控制换票，可以通过 `cowen` 提供的透明代理接口：
- **接口地址**: `POST http://127.0.0.1:8080/v1/oauth2/token`
- **请求参数**: 
  按照标准 OAuth2 协议发送 `code` 和 `grant_type=authorization_code` 即可。
- **透明增强**: `cowen` 代理会拦截此请求，**自动注入** `client_id` 和 `client_secret`，并在获取结果后**自动将永久码归档**到存储中。

### 3. 授权成功感知
虽然 `cowen` 负责了后端的令牌维护，但您的业务系统通常需要通过 `redirect_uri` 的跳转或 Webhook 转发的消息来感知授权已完成，随后即可通过代理（携带 `orgId`）开始调用该租户的 API。

---

## ⚠️ 能力边界

| 特性 | 支持状态 | 备注 |
| :--- | :--- | :--- |
| **身份类型** | ISV 多租户应用 | 支持大规模租户托管 |
| **消息推送 (Webhook)** | ✅ 全力支持 | 自动注入 `orgId` 以识别租户 |
| **死信管理 (DLQ)** | ✅ 全力支持 | 支持全量租户的消息重试 |
| **命令行 API 调用** | ❌ **不支持** | 必须通过 Proxy 模式配合 `orgId` 使用 |
| **自动化续约** | ✅ 全力支持 | 自动处理所有租户的 Token 刷新 |

## 🔐 全局存储与缓存架构 (Storage & Cache)

`cowen` 的存储架构支持从单机到生产集群的平滑演进，确保多租户身份的一致性与安全性。

### 1. 支持组件清单
- **Store (持久化层)**: 
  - `innerdb` (默认推荐): 业务审计数据存入内置 SQLite，敏感凭据锁定在本地 `.seal`。
  - `mysql` / `postgres` / `mssql`: 全量数据存入远程数据库。**推荐生产集群使用**。
  - `redis`: **[云原生]** 将 Redis 作为主持久化层（需开启持久化配置）。
- **Cache (加速层)**:
  - `none`: 无额外缓存（由数据库 `cowen_cache` 表承载令牌）。
  - `redis`: 开启分布式内存加速，进入 `HybridStore` 模式。

> [!TIP]
> **数据库连接稳定性**: 
> 在 macOS 等环境下，建议在 `db_url` 中使用 `127.0.0.1` 而非 `localhost` 以避免 IPv6 解析问题。同时，建议在连接字符串中包含显式用户名（如 `postgres://user@127.0.0.1/db`）。

### 2. 五大配置场景

| 场景 | 存储 (Store) | 缓存 (Cache) | 适用阶段 |
| :--- | :--- | :--- | :--- |
| **A (默认)** | `innerdb` (SQLite) | `memory` | 本地开发 / 侧车模式测试 |
| **B (单机扩展)** | `mysql` / `postgres` / `mssql` | `memory` | 单机生产 Sidecar (需审计) |
| **C (集群/云原生)** | `mysql` / `postgres` / `mssql` | `redis` | K8s 集群部署 / 多副本 HA |
| **D (极速模式)** | `redis` | `redis` | 高频消息同步 / 云原生无盘运行 |
| **E (极简兼容)** | `local` | `none` | Legacy。仅用于老版本 `.seal` 物理文件兼容 |

---

## 🔄 存储迁移与弹性伸缩 (Migration)

随着租户规模增长，您可以随时使用内置工具进行存储搬迁，实现“零感知”切流。

### 1. 迁移指令
```bash
# 示例：将所有租户数据从单机搬迁到生产环境 MySQL
cowen store migrate --to "mysql://user:pass@host:3306/db"
```

### 2. 核心迁移能力
- **Token 保活**: 迁移过程中，所有已授权租户的 `OrgToken` 会被同步，迁移后业务调用不中断。
- **配置同步**: 自动同步 `AppKey`, `AppSecret` 等关键凭据。
- **自动切流**: 迁移成功后，系统会自动更新本地配置并切换到新存储后端。

---

---

## 🏗️ 最佳实践：部署为 Sidecar (侧车)

在生产环境下，推荐将 `cowen` 与您的主应用部署在同一个 Pod (K8s) 或 Task (ECS) 中。

### 1. Kubernetes (K8s) 最佳实践
在 K8s 中，通过 `Deployment` 的多容器定义实现。由于 `cowen` 依赖本地持久化配置，建议使用 **Startup Script** 模式。

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: saas-gateway
spec:
  template:
    spec:
      containers:
      - name: main-app
        image: my-saas-app:latest
        env:
        - name: COWEN_PROXY
          value: "http://127.0.0.1:8080"
      - name: cowen-sidecar
        image: chanjet/cowen:latest
        # 使用环境变量驱动的一键启动模式 (One-Liner)
        # 侧车启动时会自动检测环境变量并完成隐式初始化
        command: ["cowen"]
        args: ["--profile", "isv-sidecar", "daemon", "start", "--foreground"]
        env:
        - name: COWEN_APP_MODE
          value: "store-app"
        - name: COWEN_APP_KEY
          valueFrom: { secretKeyRef: { name: cowen-secret, key: app-key } }
        - name: COWEN_APP_SECRET
          valueFrom: { secretKeyRef: { name: cowen-secret, key: app-secret } }
        - name: COWEN_ENCRYPT_KEY
          valueFrom: { secretKeyRef: { name: cowen-secret, key: encrypt-key } }
        - name: COWEN_WEBHOOK_TARGET
          value: "http://127.0.0.1:5000/callback"
        - name: COWEN_PROXY_PORT
          value: "8080"
        - name: COWEN_STORE_TYPE
          value: "mysql"
        - name: COWEN_DB_URL
          value: "mysql://user:pass@mysql-master:3306/cowen_db"
        - name: COWEN_CACHE_TYPE
          value: "redis"
        - name: COWEN_CACHE_URL
          value: "redis://redis-service:6379"
```

### 2. Docker Compose 最佳实践
```yaml
services:
  app:
    image: my-app
    ports:
      - "5000:5000" # Webhook 接收端口
    environment:
      COWEN_URL: http://127.0.0.1:8080 # 共享网络栈，直接访问 localhost
  cowen:
    image: chanjet/cowen:latest
    network_mode: "service:app" # 【关键】共享 app 的网络命名空间以绕过 SSRF 限制
    command: daemon start --foreground
    environment:
      - COWEN_APP_MODE=store-app
      - COWEN_APP_KEY=${APP_KEY}
      - COWEN_APP_SECRET=${APP_SECRET}
      - COWEN_ENCRYPT_KEY=${ENCRYPT_KEY}
      - COWEN_WEBHOOK_TARGET=http://127.0.0.1:5000/callback
      - COWEN_STORE_TYPE=redis
      - COWEN_DB_URL=redis://redis:6379
```

### 3. AWS ECS (Fargate) 最佳实践
- **容器编排**: 在同一个 Task Definition 中定义主应用和 `cowen` 容器。
- **共享命名空间**: ECS Fargate 默认在同一个任务内的容器共享 `localhost` 网络。
- **日志路由**: 建议将 `cowen` 的 `sys` 和 `audit` 日志通过 `awslogs` 驱动发送至 CloudWatch。

---

## 💡 侧车模式下的关键运维建议

1.  **无盘化与共享存储**: 侧车实例通常是易失的（Ephemeral）。**必须使用外置 MySQL/Redis** 存储租户令牌。
2.  **健康检查与监控 (v0.3.1+)**:
    *   **Liveness**: 检查 `cowen` 进程是否存在。
    *   **Readiness**: 调用 `curl -f http://localhost:8081/health`，确保管理 API 已就绪且能够连通内部存储。
    *   **Metrics**: 建议将 `http://localhost:8081/metrics` 接入 Prometheus，重点监控 `cowen_token_ttl_seconds`（租户令牌寿命）以防大面积失效。
3.  **环境体检 (System Doctor)**:
    在 Sidecar 容器启动脚本中，建议加入 `cowen doctor` 作为 Pre-flight check。
4.  **智能自适应刷新 (v0.3.1+)**: 
    `cowen` 会根据租户 Token 的剩余寿命自动平滑刷新压力，ISV 无需手动触发。
5.  **资源分配**: 
    *   `cowen` 核心使用 Rust 编写，内存占用极低。
    *   **推荐配额**: `Requests: 64MiB / 0.1 CPU`, `Limits: 128MiB / 0.5 CPU`。
4.  **优雅停机**: 
    *   确保 K8s 给 `cowen` 足够的 `terminationGracePeriodSeconds`（建议 30s），以便 Daemon 完成最后的日志上报与死信归档。
