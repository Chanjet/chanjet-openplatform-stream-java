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
  cowen config set webhook_target http://127.0.0.1:5000/callback
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

## 🌐 身份感知网关与统一路由引擎 (Inbound Gateway & Routing)

在 **v0.5.0** 中，`cowen` 引入了内置的 **Inbound 身份感知网关**。该网关作为客户端和您业务系统的第一道屏障，自动托管了用户的登录鉴权（CORS/302 重定向）、Session 状态以及多 Upstream 路由转发。

### 1. 为什么使用 Inbound Gateway？
* **零侵入登录对接**：无需在业务系统（Upstream）中编写复杂的 OAuth 回调拦截与 Cookie 读写逻辑，网关全自动搞定。
* **旁挂直连 OpenAPI**：通过统一匹配逻辑，允许客户端直连网关发送 `/open-api/**` 请求，网关在进程内自动完成加签与 3-Tier 换票自愈，免去业务系统代理中转的多跳开销。
* **多微服务分发**：支持根据 Path 分发至不同的后端微服务，并自动向后端透传已登录的租户身份（`x-org-id`、`x-user-id`、`x-app-id`）。

### 2. 网关与路由配置说明
您可以在配置文件（如 `default.yaml`）中定义网关行为及路由表：

```yaml
gateway:
  bind_address: "127.0.0.1:8080"         # 网关监听地址
  upstream_url: "http://localhost:8080"   # 兜底默认后端（向前兼容）
  
  # OAuth 登录成功后的同步 Webhook（可选）
  auth_sync_hook: "http://localhost:8080/mock_isv/auth_sync_hook"
  
  # 鉴权路由模式（STRICT-全保护 | PERMISSIVE-全放行）
  auth_routing:
    mode: "STRICT"
    bypass_rules:                         # 免登录放行白名单路径（支持 Glob）
      - "/v1/mock/ping"
      - "/static/**"
    require_rules: []
    
  # 统一路由分发规则表（从上到下匹配）
  routes:
    # 场景 A：客户端直连/旁挂 OpenAPI，网关在进程内洗刷签名并一跳直达开放平台
    - path: "/open-api/**"
      upstream: "openapi"                 # 使用 "openapi" 关键字启用旁挂直连
      strip_prefix: "/open-api"           # 转发时剥离的前缀
      
    # 场景 B：将特定前缀路由至独立的业务微服务（如订单系统）
    - path: "/order/**"
      upstream: "http://localhost:8081"
      strip_prefix: "/order"
```

### 3. 三大核心路由模式

#### 模式一：普通业务分发（ISV 微服务分发）
当请求匹配到普通 HTTP 路由规则（如 `/order/**`）时：
1. 网关验证客户端 Session 状态（不满足则根据配置返回 401 JSON 或 302 重定向到登录页）。
2. 网关剥离指定前缀后，将请求转发给指定的子微服务。
3. 网关在转发头中**自动注入租户上下文**：
   * `x-org-id`：当前会话对应的企业 ID。
   * `x-user-id`：当前会话对应的用户 ID。
   * `x-app-id`：当前应用 ID。
4. 业务微服务接收到请求后，**无需自己校验 Cookie**，直接通过 `x-org-id` 等标头即可安全地识别当前请求属于哪个租户。

#### 模式二：旁挂直连 OpenAPI（Direct OpenAPI Mode）
当请求匹配到 upstream 为 `"openapi"` 的规则（如 `/open-api/**`）时：
1. 网关拦截请求，根据当前会话自动在进程内实例化 `AuthProvider`。
2. 自动检索该租户存在 Vault 或存储中的凭据，在网关进程内直接完成请求参数的多租户加签洗刷。
3. **3-Tier 自愈换票**：网关检测到 Access Token 已失效时，会自动在进程内利用永久授权码（Permanent Code）或 Refresh Token 向开放平台静默请求新 Token，更新存储后重新完成签名，期间客户端无感知。
4. 网关将剥离前缀后的请求一跳直连真正的公网/云内开放平台接口，返回结果。

#### 模式三：兜底默认转发
对于不匹配任何 `routes` 的请求，网关将其透明反向代理至 `upstream_url` 兜底地址，并同步透传 `x-*` 租户标头。

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
  - `innerdb` (默认推荐): 业务审计数据存入内置 SQLite，敏感凭据锁定在本地 `.seal`。采用 **“双写降级”** 策略：在数据库存入全量配置的同时，会在本地 `~/.cowen/` 下镜像生成同名 `.yaml` 文件，方便开发者直观阅览或配置版本管理。
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

在生产环境下，推荐将 `cowen` 与您的主应用部署在同一个 Pod (K8s) 或 Task (ECS) 中。根据您的业务架构，Sidecar 有两种典型的部署模型：

### 1. 部署架构选型

#### 模型 A：Inbound 身份网关 + Egress 代理混合模式 (v0.5.0+ 推荐)
* **流量流向**：外部流量（客户端浏览器） -> 负载均衡/Ingress -> **`cowen` 网关端口（如 `8090`）** -> 验证 Session 并匹配路由 -> 路由给**业务主应用（如 `127.0.0.1:5000`）**。
* **安全性**：业务主应用容器只监听本地回环地址（`127.0.0.1`），完全处于网关的安全保护伞之下，不需要自己做 OAuth 和 Session 解密。
* **业务调用**：业务主应用调用外部 OpenAPI 时，仍可以通过 `cowen` 侧车代理端口（如 `8080`）做 Egress 加签。

#### 模型 B：纯 Egress 代理侧车模式 (传统模式)
* **流量流向**：外部流量 -> Ingress -> **业务主应用（暴露公网，如 `5000`）**。
* **业务调用**：业务主应用在需要调用开放平台 API 时，将代理设置指向 `cowen` 侧车端口（如 `8080`），由 `cowen` 代理完成多租户 Token 注入。

---

### 2. Kubernetes (K8s) 最佳实践

以下是启用 **Inbound 身份网关 + Egress 代理混合模式** 时的 Pod 编排配置。
我们将网关绑定在 `0.0.0.0:8090` 承接外部 Ingress 流量，把 Egress 代理绑定在 `127.0.0.1:8080` 供业务系统内调用。

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: saas-gateway
spec:
  template:
    spec:
      containers:
      # 1. ISV 主业务容器
      - name: main-app
        image: my-saas-app:latest
        ports:
        - containerPort: 5000                  # 主业务服务端口，仅监听 127.0.0.1:5000
        env:
        # 主应用向外调用 OpenAPI 时，指定 Proxy 地址指向 sidecar
        - name: COWEN_PROXY
          value: "http://127.0.0.1:8080"
          
      # 2. Cowen 侧车容器
      - name: cowen-sidecar
        image: chanjet/cowen:latest
        command: ["cowen"]
        args: ["--profile", "isv-sidecar", "daemon", "start", "--foreground"]
        ports:
        - containerPort: 8090                  # Inbound 网关端口，对外暴露
        env:
        # A. 凭据与模式驱动
        - name: COWEN_APP_MODE
          value: "store-app"
        - name: COWEN_APP_KEY
          valueFrom: { secretKeyRef: { name: cowen-secret, key: app-key } }
        - name: COWEN_APP_SECRET
          valueFrom: { secretKeyRef: { name: cowen-secret, key: app-secret } }
        - name: COWEN_ENCRYPT_KEY
          valueFrom: { secretKeyRef: { name: cowen-secret, key: encrypt-key } }
          
        # B. Inbound 网关与路由引擎配置 (v0.5.0+)
        - name: COWEN_GATEWAY_ENABLED
          value: "true"
        - name: COWEN_GATEWAY_BIND
          value: "0.0.0.0:8090"                # 网关必须监听 0.0.0.0 才能接收 Pod 外流量
        - name: COWEN_GATEWAY_UPSTREAM
          value: "http://127.0.0.1:5000"       # 默认兜底转发至 ISV 主容器
        - name: COWEN_GATEWAY_MODE
          value: "STRICT"                      # 强制所有请求均需 Session 校验
        - name: COWEN_WEBHOOK_TARGET
          value: "http://127.0.0.1:5000/callback" # 登录成功后的 auth sync Webhook 目标
          
        # C. Egress 代理及持久化配置
        - name: COWEN_PROXY_PORT
          value: "8080"                        # 本地主应用调用的代理端口
        - name: COWEN_STORE_TYPE
          value: "mysql"
        - name: COWEN_DB_URL
          value: "mysql://user:pass@mysql-master:3306/cowen_db"
        - name: COWEN_CACHE_TYPE
          value: "redis"
        - name: COWEN_CACHE_URL
          value: "redis://redis-service:6379"
```

---

### 3. Docker Compose 最佳实践

在 Compose 共享网络栈的环境中，我们同时配置了主服务、多 Upstream 路由对应的微服务容器，以及 Cowen 侧车网关。

```yaml
services:
  # 主业务服务
  app:
    image: my-app
    environment:
      COWEN_URL: http://127.0.0.1:8080         # 指向 Proxy 端口
    # 无需暴露 5000 端口，只接受本地侧车转发

  # 订单微服务
  order-service:
    image: my-order-service
    # 供网关进行多微服务分发，绑定在 localhost:8081

  # Cowen 侧车网关
  cowen:
    image: chanjet/cowen:latest
    network_mode: "service:app"                 # 共享 app 容器的网络命名空间
    ports:
      - "8090:8090"                             # 暴露 Inbound 网关端口
    command: daemon start --foreground
    volumes:
      # 可以挂载外部 default.yaml 路由表，或者使用环境变量定义
      - ./default.yaml:/root/.cowen/default.yaml
    environment:
      - COWEN_APP_MODE=store-app
      - COWEN_APP_KEY=${APP_KEY}
      - COWEN_APP_SECRET=${APP_SECRET}
      - COWEN_ENCRYPT_KEY=${ENCRYPT_KEY}
      - COWEN_STORE_TYPE=redis
      - COWEN_DB_URL=redis://redis:6379

---

### 4. AWS ECS (Fargate) 最佳实践
* **任务定义 (Task Definition)**：主业务应用和 `cowen` 容器部署在同一个 Task Definition 中。
* **Awsvpc 网络模式**：ECS Fargate 会为 Task分配一个独立的 ENI，容器之间共享 `localhost` 网络栈，内部转发极速且完全隔离。
* **端口划分**：设置 Inbound 端口映射为 `8090` 作为 ALB 的 Target Group 后端，设置 `cowen` 的 `sys` 与 `audit` 日志接入 CloudWatch 统一收集。

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
