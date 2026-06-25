# 进阶运维与自愈指南 (Operations & Resilience)

本文档整理了 `cowen` CLI 的进阶操作技巧，旨在帮助运维人员在复杂网络环境或分布式部署中保持系统稳定。

---

## 🛠️ 深度诊断与自检 (Diagnostics & Doctor)

当您发现无法接收推送或 API 调用异常时，除了基础状态检查外，建议使用 v0.3.1 引入的深度体检工具：

### 1. 一键深度体检 (System Doctor)
```bash
# 运行全面诊断（推荐在配置变更或迁移后运行）
cowen doctor

# 运行详细模式（包含插件哈希校验与网络延迟测试）
cowen doctor --verbose
```
**诊断内容包括**：
- **网络探测**：检查 OpenAPI 和 Stream Gateway 的端到端延迟。
- **存储权限**：验证数据库（SQLite/MySQL/Redis）的读写权限与表结构一致性。
- **插件校验**：检查 AI 搜索插件 (`cdylib`) 是否加载成功及其版本指纹。
- **环境隔离**：验证 `COWEN_HOME` 路径下的物理权限。

### 2. 基础状态监控
```bash
# 查看详细的身份认证与长连接状态
cowen auth status

# 检查全局存储后端与缓存的连通性
cowen store status
```

---

## 🚀 守护进程与本地网关 (Daemon & Gateway)

`cowen` CLI 的核心是基于长连接的 Streaming Gateway 守护进程。所有的本地 Webhook 转发与反向代理均由该进程持续接管。

### 1. 守护进程与开机自启 (Daemon Lifecycle & Service)
除了常规的启动停止外，`cowen` 支持将进程托管给操作系统（Systemd, Launchd, Windows Service），实现开机自启与崩溃拉起：
```bash
# 以后台静默方式启动守护进程
cowen daemon start

# 检查当前守护进程的运行状态与 PID
cowen daemon status

# 平滑热重载：在不终止长连接的情况下，热重启 Worker 子进程以应用新配置
cowen daemon reload

# 将 daemon 安装为操作系统的后台自启服务
cowen daemon service install
```

### 2. 零代码反向代理 (Local Proxy)
当守护进程启动后，会在本地开启一个反向代理端口。您可以直接向该端口发送 HTTP 请求，守护进程会自动在底层完成 AppTicket 获取、AccessToken 签发及防重放签名等操作：
```bash
# 绕过鉴权直接调用云端 API（通过本地网关代理）
curl -X POST http://127.0.0.1:8081/path/to/api -d '{"data": "value"}'
```

### 3. CLI 原生 API 调用 (Native Invocation)
除了使用本地 Proxy 发起请求外，您还可以直接利用 `cowen` 命令自带的调用客户端：
```bash
# 直接在命令行调用指定的 OpenAPI 接口
cowen api call POST /v1/some/endpoint -d '{"key": "value"}'

# 支持从本地文件读取庞大的 JSON Payload
cowen api call POST /v1/some/endpoint -f ./payload.json
```

---

## ⚡ 动态配置与热重载 (Dynamic Config & Hot-Reload)

在 v0.3.1+ 中，`cowen` 支持**零停机时间**的动态配置调整，确保生产环境下连接不断流。

### 1. 动态调整日志级别
无需重启 Daemon 进程，即可即时改变日志输出深度（用于在线排查）：
```bash
# 将日志级别动态提升至 debug (v0.3.5+ 全局配置)
cowen config set log.level debug --global
```
*注：系统会通过 `SIGHUP` 信号或内部监听器通知 Daemon 进程，变更会在 1 秒内生效。*

### 2. 指标监控端口
您可以动态修改监控端口，以适配不同的集群安全组：
```bash
# 动态修改全局监控端口
cowen config set monitor.port 9091 --global
```

---

## 📊 指标监控与健康度 API (Metrics & Health)

`cowen` 提供符合 Prometheus 标准的监控端点，支持对接 Grafana 等主流观测工具。

- **健康状态**: `GET http://127.0.0.1:8081/health` -> 返回 `UP` 状态。
- **性能指标**: `GET http://127.0.0.1:8081/metrics` -> 返回 Prometheus 格式的打点数据。

**核心指标清单**：
- `cowen_api_calls_total`: 代理调用总次数。
- `cowen_stream_reconnects_total`: 长连接重连次数（用于评估网络稳定性）。
- `cowen_dlq_size`: 当前死信队列积压数。
- `cowen_token_ttl_seconds`: 令牌剩余有效期（用于告警）。

---

## 🧩 可插拔搜索插件 (Search Plugins)

v0.3.1 引入了基于动态链接库的搜索增强架构，允许在不增加主程序体积的情况下扩展语义搜索能力。该功能完整支持 macOS, Linux 及 **Windows** 操作系统。

- **内置搜索**：默认提供基础字符串匹配。
- **AI 增强**：通过插件支持 ONNX 向量检索。
- **配置与管理**：
    - **自动发现**: 系统会自动扫描插件目录下的候选文件：
        - **macOS**: `.dylib` 或 `.so`
        - **Linux**: `.so`
        - **Windows**: `.dll`
    - **扫描路径**: 优先扫描 `/usr/local/lib/cowen/` (Unix) 或 `cowen.exe` 所在目录。
    - **精细化启闭与生命周期管理**: 
        ```bash
        # 列出本地所有的扩展插件及其状态
        cowen plugins list
        
        # 启用或禁用指定的插件（动态生效）
        cowen plugins enable cowen-search-embedding
        cowen plugins disable cowen-search-embedding
        
        # 刷新插件的 OS 级签名隔离标记（解决 macOS/Windows 下加载不受信任动态库的拦截问题）
        cowen plugins refresh-signature
        ```
    - **优雅降级**: 如果插件加载失败，系统会自动降级到 `StringMatch` 模式，确保 API 搜索功能依然可用。

---

## 📬 死信队列管理 (Dead Letter Queue)

当本地 Webhook 转发失败（如您的业务系统宕机）时，消息会进入 `DLQ`。
... (rest of content) ...


### 1. 查看待处理消息
```bash
# 查看死信摘要列表
cowen dlq list
```

### 2. 手动触发重试
在您的业务系统修复后，可以触发重试：
```bash
# 重试指定 ID 的消息 (ID 可通过 dlq list 获取)
cowen dlq retry <MSG_ID>

# 清空死信队列 (谨慎操作)
cowen dlq purge
```

---

## 🔄 权限同步与动态发现 (API Discovery)

当您在畅捷通开放平台后台修改了应用的 API 权限（如新增了某个接口的权限）时，本地缓存的规约可能需要强制刷新。

### 1. 查看与检索 API 规约
```bash
# 语义化搜索 API (意向发现)
cowen api list --search "创建订单"

# 查看某个特定 API 接口的详细输入输出规格
cowen api spec POST /v1/order/create
```

### 2. 强制权限刷新
```bash
# 强制从平台拉取最新的 OpenAPI 规约与授权白名单
cowen api list --refresh
```
*注：此操作会触发重新构建本地向量搜索索引，确保 `api list --search` 能搜到新接口。*

---

## 🗄️ 存储后端与数据迁移 (Storage & Migration)

`cowen` 默认使用基于文件的 SQLite，但在分布式多副本部署中，您可能需要迁移到统一的 MySQL 或 Redis 集群。

### 1. 后端配置与诊断
```bash
# 动态修改全局存储引擎与缓存配置
cowen store set --db-url mysql://root:pwd@127.0.0.1:3306/cowen

# 检查当前配置的主存储后端与缓存的连接性及健康状态
cowen store status
```

### 2. 安全数据迁移
```bash
# 在不同的底层存储后端之间（如 SQLite -> MySQL）安全地迁移已保存的配置、Profile 与凭据状态
cowen store migrate
```

---

## 🌐 分布式与集群一致性 (Cluster Management)

在多节点部署场景下（如 K8s ReplicaSet），多个 `cowen` 实例共用同一个 `Redis` 或 `MySQL`。

### 1. 冲突保护
`cowen` 内部实现了基于分布式锁的 **刷新仲裁机制**：
- 即使多个实例同时发现令牌即将过期，也只有一个实例会发起网络刷新请求。
- 其他实例会进入短暂等待，并随后从共享存储中直接读取新令牌。

### 2. 状态批量查看
如果您需要同时监控多个租户环境：
```bash
# 扫描并输出所有已存在的 Profile 状态
cowen status --all
```

---

## 🧹 系统一键重置与状态清理 (System Reset) (v0.3.5+)

在运维升级、迁移环境或发生重大本地存储故障时，您可能需要彻底清理本地缓存和状态，以便“恢复出厂设置”重新初始化。

`cowen` 支持插件化的二相重置清理机制，确保各组件状态的原子擦除：

### 1. 预览重置计划 (Dry Run)
在执行破坏性清除之前，强烈建议先运行 `--dry-run` 选项以获得确定性预览：
```bash
# 仅生成并输出将要删除的物理文件（数据库、日志、模型、锁）和资源清单，零副作用
cowen reset --dry-run
```

### 2. 正式执行重置
当确认无误后，即可执行无参数重置以物理抹除所有状态介质：
```bash
# 物理删除所有已注册状态，恢复出厂设置
cowen reset
```
*注：系统重置后，本地所有 Profile 将全部丢失，您需要重新运行 `cowen init` 以恢复工作能力。*

---

## 🔐 身份认证与无头环境登录 (Auth & Headless Login)

在服务器等无浏览器环境（SSH/Headless）中，直接执行登录操作无法自动弹出浏览器。`cowen` 专门为此提供了交互式的授权桥接流程。

### 1. 无头环境手动授权登录
```bash
# 1. 在服务器发起登录流程（获取授权 URL）
cowen auth login

# 2. 在本地电脑的浏览器中打开控制台输出的授权 URL 进行授权
# 3. 授权成功后，浏览器会重定向到一个无法访问的 localhost URL（这是正常现象）
# 4. 复制该无法访问的完整重定向 URL，并在服务器终端执行回调触发：
curl "<COPIED_URL>"
```
*注：回调到达本地 Proxy 后，守护进程将自动接管、写入本地保险箱并周期性刷新 Token。*

### 2. 提取原始凭证用于自动化集成
如果您需要将当前获取的凭据集成到其他自动化脚本（如 CI/CD 或外部爬虫）中：
```bash
# 以明文形式打印当前有效的 AccessToken（禁用掩码屏蔽）
COWEN_RAW_OUTPUT=true cowen auth token
```

---

## ⚙️ 应用授权模式与选型 (App Modes)

`cowen` 根据不同的业务集成架构，支持 3 种核心的 `app_mode`。在 `cowen init` 时您可以进行指定，它们在鉴权机制、人工介入度以及适用场景上有显著的区别：

### 1. 自建应用模式 (`self-built`)
- **核心场景**：企业内部系统打通、自用后台微服务集成、无 UI 的后台自动化数据流脚本。
- **鉴权机制**：基于 `AppKey` 和 `AppSecret` 的企业级静默授权。
- **能力与限制**：
  - **全自动（推荐）**：完全无需任何人工干预（不需要浏览器），可以直接由后台守护进程安全换取并自动续签 AccessToken。
  - **分布式友好**：支持在云端或 Kubernetes 中通过 Docker 进行多节点容器化部署，其无状态特质允许共用统一的 MySQL/Redis 集群。

### 2. 商业化/商店应用模式 (`store-app`)
- **核心场景**：ISV（独立软件开发商）开发的标准化 SaaS 应用，需上架畅捷通应用商店供全网多租户购买使用。
- **鉴权机制**：基于开放平台主动网关推送的动态 `AppTicket` 机制。
- **能力与限制**：
  - **网关/Ticket 驱动**：此模式强制要求启动并依赖 `cowen daemon` 的本地或公网 Webhook 网关（Gateway），系统必须先接收开放平台定时推送的 `AppTicket`，才能去换取 Token。
  - **原生多租户架构**：具备最完备的商用环境机制，支持应对高并发的租户级复杂回调与状态机流转。

### 3. 三方个人授权模式 (`oauth2`)
- **核心场景**：开发者个人调试工具、供单个用户使用的桌面端辅助工具。
- **鉴权机制**：标准的三方 OAuth2 授权码 (Authorization Code) 模式。
- **能力与限制**：
  - **强人工介入**：首次登录必须强依赖浏览器跳转，并在畅捷通开放平台页面人工点击授权同意（Headless 服务器必须通过上一节提到的 `cowen auth login` 交互流完成）。
  - **单机隔离限制**：由于强依赖个人维度的用户授权上下文，为了安全起见，`cowen` 内部通过代码校验**严格禁止**在分布式多存储环境（共享数据库）中混用 `oauth2` 模式（CLI 将直接报错拒绝），它仅适合用于单机 Sidecar 或开发者的工作站。

---

## 👥 多环境/多租户隔离 (Profile Management)

`cowen` CLI 采用 Profile 机制来实现多应用、多租户的安全隔离，类似 AWS CLI 或 kubectl 的 context。

### 1. 切换与管理 Profile
```bash
# 列出本地所有已初始化的 Profile
cowen profile list

# 切换当前激活的默认 Profile
cowen profile switch <PROFILE_NAME>

# 获取当前正在生效的 Profile
cowen profile current
```
*注：在任何其他命令中，都可以通过 `-p <PROFILE>` 或设置环境变量 `COWEN_PROFILE` 临时指定运行环境而不修改默认激活状态。*

---

## 🔍 可观测性与日志审计 (Observability & Audit)

为了追踪系统的流转历史，`cowen` 提供了极其细粒度的内置日志与事件回溯能力。

### 1. 查看运行日志
您可以随时追踪底层服务的网络交互、数据库读写以及插件加载细节：
```bash
# 查看并动态追踪当前 daemon 进程日志（类似于 tail -f）
cowen log
```

### 2. 追踪业务审计日志
当我们需要回溯到底是哪个 API 被调用、以及 Webhook 消息是否成功投递时：
```bash
# 查看核心业务结构化审计事件
cowen audit
```

### 3. 系统级事件回溯
对于更细粒度的生命周期事件与故障诊断轨迹：
```bash
# 列出过去发生的系统事件流与故障轨迹
cowen events
```

---

## ⌨️ 效率工具 (Efficiency)

### 1. 命令补全 (Shell Completion)
支持 Zsh, Bash, Fish 和 PowerShell 的自动补全：
```bash
# 以 Zsh 为例，安装补全脚本
cowen completion --install
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
