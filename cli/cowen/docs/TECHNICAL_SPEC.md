# cowen 技术规约文档 (Technical Specification v0.3.5)

本文档旨在定义 `cowen` CLI 的核心技术实现标准、底层算法及协议细节，作为开发与评审的执行依据。

---

## 1. 核心架构约束 (Core Architecture)

### 1.1 开闭原则 (OCP) 落地
- **SPI 机制**: 所有可变逻辑（鉴权模式、存储后端、流量处理器）必须抽象为 Trait。
- **动态发现**: 采用 `inventory` 宏实现插件的零修改注册。新增 Provider 只需在相应目录下实现 Trait 并调用 `inventory::submit!`。

### 1.2 异步并发模型
- **Runtime**: 基于 `tokio` 多线程运行时。
- **并发控制**: 
    - 内存级：使用 `Arc<Mutex<T>>` 或 `tokio::sync` 原语。
    - 物理级：关键操作（如 Token 刷新）必须持有 **跨进程文件锁 (File Lock)**。
    - **分布式**: 支持通过 `Store` 实现分布式互斥逻辑（乐观锁/版本号）。
- **Profile 唯一性约束**: `AppKey` + `AppMode` 的组合在系统中必须具备全局唯一性，禁止创建指向同一云端实例的重复 Profile。

---

## 2. 存储与安全规约 (Storage & Security)

### 2.1 存储域划分 (Store Domains)
系统将数据持久化划分为五个物理隔离的域：
- **Config**: 存放非敏感 manifest 配置。
- **Secret**: 存放 AppSecret、证书等敏感凭据（强制加密）。
- **Token**: 存放各级 AccessToken 及授权码（强制加密，具备 TTL）。
- **Audit**: 存放结构化审计流水（只增不减）。
- **DLQ**: 存放死信消息，支持优先级。
- **Tenant Context**: 在 `StoreApp` 模式下，针对不同 `org_id` 物理隔离其令牌归档条目，确保多租户环境下令牌检索的确定性。

### 2.2 Vault 加密体系
`Vault` 是处理所有机密数据的逻辑外壳：
- **算法**: AES-256-GCM (Authenticated Encryption)。
- **密钥派生**: 基于 `machine-id` (机器指纹) 与内部固定盐值 (Salt) 派生。
- **存储安全性**: 防止凭据在不同物理机之间通过拷贝 `~/.cowen` 目录直接复用。

### 2.3 混合存储与缓存一致性 (Hybrid Storage)
- **协作模型**: Persistence (SQL) 为真值来源，Cache (Redis) 为加速层。
- **失效策略**: 采用 `Write-Through` 模式，所有更新操作必须同步至 SQL 和 Redis。
- **数据漂移处理**: 系统目前依赖缓存 TTL 自然失效实现同步。在分布式环境下，若 SQL 数据被外部修改，Cache 可能存在短时间的过期数据（已知盲区，需结合 E2E Case 26 规约）。

---

## 3. 消息推送与守护进程 (Streaming & Daemon)

### 3.1 稳定连接与启动策略
- **长连接**: 维持与开放平台的双向 WebSocket 通信。
- **自愈机制**:
    - **Heartbeat**: 30s 周期探测。
    - **Reconnection**: 采用指数级退避算法 (`Initial: 1s, Max: 60s, Factor: 2.0`)。
- **守护进程启动重试 (Daemon Boot & IPC Ping)**: 
    - 鉴于后台进程启动（尤其是在低端机器或初始化大型 SQLite WAL 结构时）可能产生延迟，`ensure_daemon` 调用采用容忍策略：首帧 Ping 允许包含最高达数秒的动态延迟重试阈值，通过抗抖动机制防止由于超时误判导致的进程重复 Fork 衍生。
- **动态端口回退策略 (Port Fallback)**:
    - 针对 `monitor_port`，默认配置为 `0`（表示启用自动回退探测）。当默认探针端口（或随机端口）受阻时，会自动降级申请其他可用空闲端口。
    - 若用户显式配置为非 0 的静态端口，则遵循 **Fail-Fast** 原则，若端口被占用将直接拒绝启动并输出明确的端口占用错误。

### 3.2 转发与死信 (Forwarding & DLQ)
- **转发重试**: 消息推送失败后，进入重试队列。
- **死信判定**: 超过 5 次重试仍失败的消息，归档至 `DLQ` 域。
- **离线持久化**: 保证 Daemon 进程重启后，未处理完的推送任务可断点续传。

### 3.3 连接互斥与独占规范 (Exclusivity)
- **策略**: 采用“后浪推前浪” (Last One Wins) 策略。
- **行为**: 当 Profile 开启 `exclusive` 模式启动时，若云端已存在相同 AppKey 的连接，新连接将强制导致旧连接被踢出。

### 3.4 集群幂等性规约 (Clustered Idempotency)
- **转发去重**: 在共享存储集群模式下，系统必须通过数据库行级锁或 Redis 互斥锁确保同一个 `msgId` 在转发给本地 Sink 时仅能被成功处理一次。

---

## 4. 鉴权与生命周期 (Auth Lifecycle)

### 4.1 令牌自动维护
- **检测阈值**: 当令牌剩余有效时间 `< 15分钟` 或 `< 总寿命的 10%` 时，触发主动续约。
- **原子刷新**: 刷新过程中阻塞后续请求（等待锁），确保平台侧不会因并发刷新导致 `RefreshToken` 失效。

### 4.2 流量劫持规范
- **Proxy 拦截**: 在本地代理模式下，根据 API 规约自动注入 `openToken` 或 `Signature`。
- **Webhook 伪造防护**: 验证来自平台的摘要信息（使用 `encrypt_key`），确保转发任务的合法性。

---

## 5. 语义引擎规范 (Neural Engine)

- **Embedding 模型**: BGE-small-zh-v1.5 (量化为 ONNX 格式)。
- **推理引擎**: `onnxruntime` (ort)，限制单线程执行以减少 CLI 资源占用。
- **索引策略**: 
    - 本地构建基于 Cosine Similarity 的线性搜索。
    - 缓存规约 MD5 校验，避免重复执行长文本向量化。
- **兼容性 (Compatibility)**: 
    - **AVX 依赖**: `onnxruntime` 默认需要 CPU 支持 AVX 指令集（如 Intel Core 系列）。
    - **Legacy 模式**: 对于不支持 AVX 的旧款或低功耗 CPU（如 Intel Pentium G5400, Celeron 等），提供 **Legacy Build**。
    - **功能差异**: Legacy 版本通过禁用 `ai` Feature 编译，将不支持语义搜索 (`api list -s`)，但保留所有核心鉴权、代理与 Streaming 桥接能力。

---


## 6. 日志、审计与遥测 (Observability)

### 6.1 日志分层
| 标识 (Target) | 格式 | 目的 |
| :--- | :--- | :--- |
| `sys` | JSON/Text | 系统诊断、启动链路追踪。 |
| `audit` | JSON | 记录所有的调用元数据（Path, Method, Success, UserID）。 |
| `stream` | JSON | 推送稳定性监控。 |

### 6.2 遙测规范
- **上报时机**: 关键命令执行结束、严重异常发生。
- **采样策略**: 生产环境采取 10% 采样，`DEBUG` 环境 100% 上报。
- **数据脱敏**: 严禁上报任何包含令牌片段、密钥或用户业务数据的 Payload。

---

## 7. 编译与运行环境变量规约 (Build & Runtime Env) (v0.3.5+)

为了保障配置的安全隔离与构建的确定性，`cowen` 定义了严格的编译期与运行期环境变量契约：

### 7.1 编译期强校验注入
必须在编译期通过构建脚本（如 `build.rs`）校验并静态注入以下变量，任何缺失或格式错误均会导致编译失败：
- `COWEN_BUILD_CLIENT_ID` (或 `BUILTIN_CLIENT_ID`): 编译期注入的内置自建应用 Client ID，用于安全引导和默认代金券交换。
- `DEF_MARKET_URL`: 编译期注入的默认开放平台应用市场 API 地址。

### 7.2 运行时全局环境变量覆盖
运行时允许通过带 `COWEN_GLOBAL_` 前缀的环境变量直接覆盖全局 `app.yaml` 中的对应配置项，无需修改物理文件即可调整行为：
- `COWEN_GLOBAL_LOG_LEVEL`: 覆盖全局日志级别（如 `sys` 日志输出级别）。
- `COWEN_GLOBAL_SSRF_WHITELIST`: 覆盖 Webhook 转发的 IP/网段白名单。
- `COWEN_GLOBAL_VAULT_PIN`: 覆盖运行时 Vault 指纹 PIN 码。

---
© 2026 Chanjet Advanced Agentic Coding Team.
