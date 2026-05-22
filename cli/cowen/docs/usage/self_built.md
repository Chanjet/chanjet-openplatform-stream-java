# Self-Built 模式使用指南 (企业自建模式)

Self-Built 模式是畅捷通为企业自有系统集成提供的**深度定制方案**。它通过 `AppKey` / `AppSecret` 结合平台每 20 分钟推送一次的 `AppTicket` 机制，实现最稳定的单租户 API 调用与实时消息订阅。

## 🎯 目标场景
- **企业集成**：ERP/OA 系统与畅捷通产品实现深度数据打通。
- **实时同步**：需要接收并处理实时业务消息（如：订单下单推送）。
- **长期运行**：适合部署在服务器上，作为长期运行的消息中转站。

## 🚀 核心工作流

### 1. 初始化配置 (四要素强制)
初始化自建模式时，您必须提供以下关键凭据：
```bash
cowen init --profile corp-dev \
           --app-mode self-built \
           --app-key <YOUR_APP_KEY> \
           --app-secret <YOUR_APP_SECRET> \
           --certificate <YOUR_CERTIFICATE> \
           --encrypt-key <YOUR_ENCRYPT_KEY>
```
> [!NOTE]
> **构建期内置 AppKey 简化方案 (v0.3.5+)**：
> 如果您使用了通过构建期静态注入（基于 `COWEN_BUILD_CLIENT_ID` 环境变量）预编译的特定企业级版本包，则在执行 `cowen init` 时可以无需显式提供 `--app-key` 参数，系统将自动使用编译期校验并注入的内置应用标识进行安全引导。
> [!IMPORTANT]
> **安全性与存储模式 (Vault)**：
> 您的 `AppSecret` 和 `EncryptKey` 将存储在 **Vault** 中。系统采用 **设备指纹加密 (Machine-Fingerprint Encryption)** 方案，基于主机名、系统版本等硬件信息派生密钥，确保凭据在物理层面被隔离保护。

### 2. 验证初始化状态
初始化成功后，系统会**自动启动**后台守护进程。请运行以下命令检查：
```bash
cowen status
```
在输出中，您应重点关注：
- **Auth Status**: 确认已成功获取 `App Access Token`。
- **Stream Bridge**: 确认状态为 `ACTIVE` (表示已建立 WebSocket 长连接，正在等待 AppTicket 推送)。

### 3. 关于守护进程 (Daemon)
> [!TIP]
> **自动启动行为**：不同于 OAuth2 模式，Self-Built 模式在 `init` 成功后会**立即拉起**后台守护进程。

**为什么必须保持 Daemon 运行？**
- **智能自适应刷新 (v0.3.1+)**：令牌刷新不再依赖硬编码的时间，而是基于 Token 剩余寿命的 **80% 规则** 自动计算下一次检查时间，并配合 **随机抖动 (Jitter)**。这确保了在成千上万个节点同时运行的情况下，不会对平台造成突发流量冲击。
- **令牌续约**：该模式的令牌刷新极其依赖 `AppTicket`。而 `AppTicket` 只能通过长连接实时接收。如果 Daemon 关闭，系统将无法接收新 Ticket，导致令牌最终无法刷新。
- **本地代理**：提供 `localhost:8080` 代理服务。

### 4. 环境体检与诊断 (v0.3.1+)
在企业生产环境中，建议定期或在故障时运行自检工具：
```bash
cowen doctor --profile corp-dev
```
该工具会深度验证当前环境的证书合法性、存储读写权限以及与平台的实时连通性。

### 4. 本地代理 (Proxy) 使用指南
在自建模式下，本地代理是实现“无感调用”的核心工具。

- **默认行为**：初始化后，代理服务默认处于 **开启** 状态。
- **端口机制**：系统在初始化时会**自动寻找一个可用端口**。您不再需要担心默认 8000 端口冲突。
    - **如何查看**：运行 `cowen status` 查看 `Proxy:` 这一行标注的实际端口。
    - **如何手动指定**：如果您有固定端口需求，可以通过全局修改指令进行调整：
      ```bash
      cowen config set proxy.port <PORT> --global
      ```
- **开启/关闭控制**：
    - **持久化关闭**：
      ```bash
      cowen config set proxy.enabled false --global
      ```
    - **临时关闭启动**：`cowen daemon start --disable-proxy`
- **发起调用**：
  ```bash
  # 代理会自动注入符合规范的 openToken 和 appKey
  curl http://127.0.0.1:8080/v1/user/info
  ```
  > [!IMPORTANT]
  > **身份注入说明**：代理会自动在请求头中注入 `openToken` 和 `appKey`。您仅需提供业务相关的 Header（如 `Content-Type: application/json`）。

### 5. 消息推送与 Webhook 转发
`cowen` 可以将接收到的实时业务消息（如：订单变更、审批流提醒）自动转发给您的业务系统。

- **配置转发地址**：
  在初始化时或通过全局配置命令设置目标 URL：
  ```bash
  cowen config set security.webhook_target http://127.0.0.1:3000/api/callback --global
  ```
  > [!CAUTION]
  > **安全约束 (SSRF 防护)**：出于安全合规要求，`webhook-target` **仅支持本地回环地址** (`127.0.0.1` / `localhost` / `[::1]`)。严禁指向任何外部域名或非本机的内网 IP。
- **工作原理**：
    1. 守护进程通过 WebSocket 实时接收消息。
    2. **系统消息过滤**：`cowen` 会自动拦截并处理 `AppTicket` 和 `TempAuthCode` 等系统级消息（用于自动续约），**这些消息不会转发给您的 Webhook**。
    3. **业务消息转发**：对于业务类消息，`cowen` 会立即发起一个 **HTTP POST** 请求到您的 `webhook-target`。
- **可靠性保障 (DLQ)**：
    如果您的业务服务器暂时不可用（返回非 200 状态码），该消息会自动进入**本地死信队列**。您可以在服务器恢复后运行 `cowen dlq retry` 进行重试。

## 🔐 全局存储与缓存架构 (Storage & Cache)

`cowen` 的存储架构旨在适应从个人开发环境到企业级 K8s 集群的无缝迁移。

### 1. 支持组件清单
| 场景 | 存储 (Store) | 缓存 (Cache) | 适用阶段 |
| :--- | :--- | :--- | :--- |
| **A (默认)** | `innerdb` (SQLite) | `memory` | 本地开发 / 小规模测试 |
| **B (单机扩展)** | `mysql` / `postgres` / `mssql` | `memory` | 单机生产环境 (需审计) |
| **C (集群/云原生)** | `mysql` / `postgres` / `mssql` | `redis` | 大规模集群 / 生产多节点 |
| **D (极速模式)** | `redis` | `redis` | 高频访问 / 容器化无盘运行 |
| **E (兼容模式)** | `local` (文件) | `memory` | 旧版升级兼容 |

- **Store (持久化层)**: 
  - `local` (默认): 数据以加密形式存放在 `~/.cowen/.seal` 目录下。无需任何安装，开箱即用。
  - `innerdb`: 业务数据（日志、队列）存储在本地 SQLite 数据库中，敏感凭据锁定在本地 `.seal`。
  - `mysql` / `postgres` / `mssql`: 全量数据存入远程数据库。**推荐生产集群使用**，支持多节点共享身份。
- **Cache (加速层)**:
  - `none` (默认): 无额外缓存。
  - `redis`: 开启分布式令牌缓存，使用 `HybridStore` 混合存储方案。

### 2. 五大配置场景

#### 🟢 场景 A：单机开发 (默认推荐)
**说明**：**新装默认模式**。采用 `innerdb` 方案：业务数据（审计、DLQ）存入内置 SQLite，敏感凭据由本地 `.seal` 文件加密锁死。
- **配置**: `store: innerdb`, `cache: none`
- **优势**：开箱即用，支持 SQL 级消息审计与死信管理。

#### 🟡 场景 B：仅外置数据库 (高可用持久化)
**说明**：适用于多机部署。数据持久化与令牌缓存均由数据库承载（利用内置 `cowen_cache` 表）。
- **配置**: `store: <DB_TYPE>`, `cache: none`
- **命令**: `cowen store set --store mysql --db-url "..."`

#### 🟠 场景 C：生产级全家桶 (DB + Redis)
**说明**：**集群部署的最优解**。持久层保证一致性，Redis 提供高性能令牌缓存。
- **配置**: `store: <DB_TYPE>`, `cache: redis`
- **命令**: `cowen store set --store mysql --db-url "..." --cache redis --cache-url "..."`

#### 🔴 场景 D：纯 Redis 模式 (云原生)
**说明**：完全不依赖本地文件。需确保 Redis 开启持久化（AOF/RDB）。
- **配置**: `store: redis`, `cache: none`
- **命令**: `cowen store set --store redis --db-url "redis://..."`

#### ⚪ 场景 E：极简兼容模式 (Legacy)
**说明**：仅用于老版本迁移。数据全量存放在本地 `.seal` 加密文件中。
- **配置**: `store: local`, `cache: none`

> [!TIP]
> **连通性检查**：配置完成后，请务必运行 `cowen store status`。系统会自动对数据库和 Redis 进行 PING 测试，确保底座稳固。

---

## 🔄 数据迁移 (Data Migration)

如果您需要从单机环境（`innerdb` / `local`）搬迁到生产集群（`mysql` / `postgres` / `redis`），`cowen` 提供了内置的无痛迁移工具。

### 1. 迁移指令
使用 `store migrate` 指令将当前存储的数据全量同步到目标存储：

```bash
# 示例：从本地搬迁到 MySQL
cowen store migrate --to "mysql://user:pass@host:3306/db"
```

### 2. 迁移模式说明
- **`clone` (默认)**: 同步全量数据并切换配置，原有的本地数据依然保留。
- **`move`**: 同步完成后，自动清理源端的旧数据（推荐在生产环境回收单机磁盘空间时使用）。
  ```bash
  cowen store migrate --to "mysql://..." --mode move
  ```

### 3. 同步内容清单
迁移过程会自动搬迁以下资产：
- ✅ **租户配置**: 所有 Profile 的基础配置。
- ✅ **敏感凭据**: `AppSecret`, `Certificate`, `EncryptKey` 等加密资产。
- ✅ **访问令牌**: 所有活跃的 `OrgToken` (迁移后继续有效)。
- ✅ **审计日志**: 最近 5000 条操作流水。
- ✅ **死信队列**: 待处理的失败消息。

---

## ⚠️ 能力边界
- **身份类型**：企业自建应用 (单租户)。
- **消息推送**：✅ 完美支持 WebSocket 长连接及 Webhook 转发。
- **死信管理**：✅ 自动托管失败消息，支持使用 `dlq list` 和 `dlq retry` 运维。

---

## ☁️ 云原生与分布式部署 (Cloud-Native & Distributed)

在生产环境中，建议采用 **Sidecar (边车模式)** 部署。通过环境变量注入配置，实现 Pod 启动即就绪。

### 1. 推荐：环境变量驱动 (One-Liner)
利用 `cowen` 的环境变量注入能力，您可以无需手动执行 `init` 脚本，直接通过容器配置启动：

**核心环境变量 (Self-Built 模式)**:
- `COWEN_APP_MODE`: 设为 `self-built`
- `COWEN_APP_KEY`: 填写应用 AppKey
- `COWEN_APP_SECRET`: 填写应用 AppSecret
- `COWEN_CERTIFICATE`: 填写应用签名证书 (Certificate)
- `COWEN_ENCRYPT_KEY`: 16位物理加密密钥
- `COWEN_STORE_TYPE`: 设为 `redis` (分布式场景必备)
- `COWEN_DB_URL`: `redis://<HOST>:<PORT>/<DB>`
- `COWEN_WEBHOOK_TARGET`: Webhook 回调地址

### 2. Kubernetes Sidecar 部署示例
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-isv-app
spec:
  template:
    spec:
      containers:
      - name: main-app
        image: my-business-app:latest
      - name: cowen-sidecar
        image: chanjet/cowen:latest
        command: ["cowen", "daemon", "start", "--foreground"]
        env:
        - name: COWEN_APP_MODE
          value: "self-built"
        - name: COWEN_APP_KEY
          value: "<YOUR_APP_KEY>"
        - name: COWEN_APP_SECRET
          valueFrom: { secretKeyRef: { name: cowen-secret, key: app-secret } }
        - name: COWEN_CERTIFICATE
          valueFrom: { secretKeyRef: { name: cowen-secret, key: certificate } }
        - name: COWEN_ENCRYPT_KEY
          valueFrom: { secretKeyRef: { name: cowen-secret, key: encrypt-key } }
        - name: COWEN_STORE_TYPE
          value: "redis"
        - name: COWEN_DB_URL
          value: "redis://redis-cluster:6379/0"
        - name: COWEN_PROXY_PORT
          value: "8080"
```

### 3. 分布式一致性保障
- **Token 共享**: 当 `COWEN_STORE_TYPE` 设为 `redis` 时，所有 Pod 将共享同一个 AccessToken。
- **并发安全**: `cowen` 内部通过 Lua 脚本实现 CAS 原子操作。即使 100 个 Pod 同时启动，也只会有一个 Pod 执行真正的 Token 交换，其余 Pod 会自动等待并复用缓存中的有效 Token。
- **无感扩容**: 由于底座是共享的，您可以随时对应用进行 HPA 扩容，新节点上线后会自动完成自愈初始化。
- **健康检查**: 建议配置 Liveness Probe 指令：`cowen status --profile main`。
