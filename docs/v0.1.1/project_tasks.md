# v0.1.1 项目执行任务规划 (Project Task Plan)

> **版本**：v0.1.1  
> **目标**：交付具备核心鉴权降压、双轨 Webhook 倒灌及死信容灾能力的 cjtCli 工具。  
> **核心原则**：TDD 强制、接口解耦、Agent 友好。

## 阶段一：核心基座构建 (Core Infrastructure)
本阶段目标是建立所有静态和无状态的基础组件，为后续复杂的逻辑提供支撑。

- [ ] **1.1 配置编排语义实现 (`core/config`)**
  - 使用 Viper 处理多 Profle 加载。
  - 实现 `Watch` 机制支持日志等级热更新。
  - **验收点**：通过 `Manager` 接口获取参数，不直接读取 OS 文件。
- [ ] **1.2 物理秘钥冷库实现 (`core/vault`)**
  - 集成 `keyring`。
  - 实现 AES-GCM-256 降级加密方案（`.seal` 文件）。
  - **验收点**：在 macOS  keychain 无法使用时能自动回切且测试通过。
- [ ] **1.3 静流输出拦截器 (`core/telemetry`)**
  - 集成 `zap` 和 `lumberjack` 实现分域滚动。
  - 实现命令级 `Recover` 钩子，将 Panic 重塑为 JSON。
  - **验收点**：`Stderr` 无控制台转义码，仅输出结构化错误。

## 阶段二：鉴权命脉与状态机 (Auth State Machine)
本阶段是系统最高复杂度的逻辑点，必须通过高并发 Mock 验证。

- [ ] **2.1 单兵阻挂锁 (Single-Flight Barrier) 实现**
  - 确保多个并发请求在 Token 失效时只触发一次刷新。
- [ ] **2.2 内存票据池与寻活推流回调**
  - 维护 `appTicket` 的内存生命周期。
- [ ] **验收点**：高并发压测下没有产生“惊群效应”，Token 刷新逻辑原子化。

## 阶段三：长链隧道与事件总线 (Daemon & Stream)
实现常驻后台的核心连接能力。

- [ ] **3.1 拨号器实现 (`daemon/stream`)**
  - 实现 WebSocket/TCP 长连接与自动重连。
  - 实现报文脱壳与鉴权清洗。
- [ ] **3.2 事件分发回调订阅机制**
  - 将 `appTicket` 事件路由至 `core/auth`。
  - 将 `webhook` 工作事件路由至 `daemon/proxy`。
- [ ] **验收点**：断网后能自动触发指数退避重连，且不丢失底层连接上下文。

## 3. 阶段四：倒灌代理与死信隔离 (Proxy & DLQ)
实现业务流量的最后一步分发与兜底。

- [ ] **4.1 双轨倒灌逻辑实现**
  - 根据 `webhook.target` 静态分发。
  - 实现临时插拔引流 (`proxy start`) 动态接口。
- [ ] **4.2 三段式阶梯抢救算法实现**
  - 实现瞬间重锤 -> 指数退避 -> SQLite 沉降。
- [ ] **4.3 死信管理指令 (`dlq list/retry`)**
  - 实现从 SQLite 捞起报文并重新注入 Proxy 发射管。
- [ ] **验收点**：靶机关机时，事件能准确写入 SQLite；靶机开机后，手动 retry 能成功送达。

## 阶段五：入口组装与 Agent 适配 (CLI & Integration)
将所有零散的接口组装成 `cjtCli` 命令。

- [ ] **5.1 命令树全景搭建 (Cobra)**
  - 实现 `config`, `auth`, `api`, `proxy`, `daemon`, `dlq` 全系子命令。
- [ ] **5.2 API 列表与语义搜索功能**
  - 实现 `api list --remote` 在线抓取。
  - 集成向量检索逻辑（可选）。
- [ ] **验收点**：`cjtCli auth status` 输出符合 Agent 交互规约的 JSON。

## 验收流程 (Definition of Done)
1. **测试覆盖**：所有核心接口必须具备 `Mock` 测试，行覆盖率 > 80%。
2. **规范检查**：日志输出绝对无颜色转义符，所有错误信息必须带 `suggestion`。
3. **性能指标**：`api call` 鉴权开销在缓存命中时 < 1ms。
