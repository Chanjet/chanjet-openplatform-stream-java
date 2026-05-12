# cowen 技术规约文档 (Technical Specification v0.3.0)

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
    - 分布式：支持通过 `Store` 实现分布式互斥逻辑（乐观锁/版本号）。

---

## 2. 存储与安全规约 (Storage & Security)

### 2.1 存储域划分 (Store Domains)
系统将数据持久化划分为五个物理隔离的域：
- **Config**: 存放非敏感 manifest 配置。
- **Secret**: 存放 AppSecret、证书等敏感凭据（强制加密）。
- **Token**: 存放各级 AccessToken 及授权码（强制加密，具备 TTL）。
- **Audit**: 存放结构化审计流水（只增不减）。
- **DLQ**: 存放死信消息，支持优先级。

### 2.2 Vault 加密体系
`Vault` 是处理所有机密数据的逻辑外壳：
- **算法**: AES-256-GCM (Authenticated Encryption)。
- **密钥派生**: 基于 `machine-id` (机器指纹) 与内部固定盐值 (Salt) 派生。
- **存储安全性**: 防止凭据在不同物理机之间通过拷贝 `~/.cowen` 目录直接复用。

---

## 3. 消息推送与守护进程 (Streaming & Daemon)

### 3.1 稳定连接策略
- **长连接**: 维持与开放平台的双向 WebSocket 通信。
- **自愈机制**:
    - **Heartbeat**: 30s 周期探测。
    - **Reconnection**: 采用指数级退避算法 (`Initial: 1s, Max: 60s, Factor: 2.0`)。

### 3.2 转发与死信 (Forwarding & DLQ)
- **转发重试**: 消息推送失败后，进入重试队列。
- **死信判定**: 超过 5 次重试仍失败的消息，归档至 `DLQ` 域。
- **离线持久化**: 保证 Daemon 进程重启后，未处理完的推送任务可断点续传。

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
© 2026 Chanjet Advanced Agentic Coding Team.
