# 畅捷通 Openplatform Stream Connector CLI - 技术架构设计 (v0.1.1)

本文档基于 PRD (v0.1.1) 及技术栈选型结论，自顶向下对 CLI 的系统边界、核心组件模块、物理目录树规划及关键数据时序进行底层架构设计，作为后续编码研发的纲领蓝图。

---

## 1. 宏观系统上下文图 (Context Boundaries)

CLI 工具在整个开发与调试生态中扮演**“安全沙箱网关与多核调度代理”**的角色。

- **顶层触发者**：
  - **Human Developer (人类开发者)**：通过终端 TTY 发起交互、查阅文档或拉起驻留程序。
  - **AI Agent (如 OpenClaw)**：通过 SubProcess (子进程) 或 Shell 执行环境，以严格禁止强交互提示框的并行状态 + `--format json` 模式拉起静默检索与执行请求。
- **底层依赖与通信靶机**：
  - **Open Platform API (开放平台接口)**：提供核心业务数据的标准 RESTful 短频响应服务。
  - **Connector Server (网关服务器)**：提供事件即时分发的底层流式消息长链接底座（依托工作空间内自有 SDK）。
  - **Local App / Legacy Systems (本地老旧微服务群)**：被局域网 `127.0.0.1` 反向代理层安全包裹的老旧业务；以及负责承接该网关所过滤处理的分发 Webhook 推流。

---

## 2. 核心模块与抽象隔离分层 (Layered Architecture)

整套 CLI 工具在包空间与代码组织上应严格分为表现层、业务控制流层、及基建安全底座层：

### 2.1 用户态交互表现层 (Presentation Layer)
- **Cobra 调度树栈**：解析 `init`, `api`, `proxy`, `webhook` 等根命令，并进行参数 (Flags & ENV) 的全绑定防腐校验。
- **Formatter 渲染引擎**：强力拦截系统发往 `stdout` / `stderr` 的每一条数据。根据全局参数决定它是要渲染出具有人类斑斓语法高亮观感的 ASCII Table/Log 瀑布树，还是直接转换为完全供机器模型解析的 Strict JSON 甚至 YAML 结构化强文本；并在出错时硬性附带 `Recovery Strategy` (纠偏指令建议)。

### 2.2 核心业务调度层 (Business Dispatch Layer)
- **Profile Context Engine (单体多租户状态机)**：以单例模式维护本次激活中的隔离期环境上下文态。严苛阻断所有平行上游业务流直接对底层 Keyring 凭证或配置文件实施直读直写动作。当前环境被架构强制锁定为唯一的单向落地区间即 `self-built`（自建企业应用体系）。凡涉及扩充第三类生态系统接入的多态扩展口规划，均已被剥离并移入《暂缓演进清单》。
- **Agent Skill Semantic 引擎 (高阶语义护城河)**：
  - 执行 OpenAPI 文档规约长文本向量化的序列化 (Serialization) / 反序列化提取。
  - 内嵌挂载极轻量级 ONNX Runtime，在常驻 Daemon 的闲时态预读浮点矩阵库。
  - 核心处理引擎直接用原生的纯内存 Float 算法算子执行高暴力的 Top-K Cosine (余弦相似度) 计算排名。
- **Net-Daemon 多工复用器 (Proxy & Webhook Daemon)**：
  - **Proxy Worker**：基于 `httputil.ReverseProxy` 拦截网络并限制绑定回环的套接字空间 (`127.0.0.1`)，透明接管流量并挂载安全加密请求头往远端发送。
  - **Stream Receiver**：调取官方私有 SDK 把持长连接接收消息帧。内置一套状态观测器，当发生本地 Target Webhook 下探不达（超时报错）且退避超限后，执行死信（DLQ）重发封存动作。

### 2.3 基础设施及安全管控层 (Infra & Security Matrix)
- **极光盾中间件 (Security & Cryptography Interceptor)**：
  - **动态拔票引擎 (Ticket Bootstrap)**：内置一个具备开机自检能力的探针心跳。每当 Daemon 常驻守护被拉起（无论是冷启动还是崩溃重启），探针扫视本地存储。如果未发现存活期的 `appTicket`，它将强行钳制后续的 Token 刷新或对外请求，并向开放平台上游**主动打出光速发票口令**（触发平台立刻定向投石问路推送一条包含新 Ticket 的 Stream 报文），拿到后即可落款 `Ready` 放行系统，从物理上断绝了纯靠“干等”导致的系统几十分钟的无响应真空期。
  - **无缝保活与短命换签**：用该自持的无暇 `appTicket` 再去申请或自动续期短命的 `openToken`，利用“短时效临牌流转机制”对核心信道隔离降险。
  - **TLS Firewall 强网屏障**：全面接管重写 Go 底层的 `http.Transport`。对任何发出开放平台域网的 Socket 报文前置注入**“3D 交叉安检门”**：
    1. 不采用动态，依靠硬编码商业默认 CA 数组池比对。
    2. 无情绞杀带有 `*` 星号的潜在过失泛域名受灾证书。
    3. 解析强审证书名是否以根红苗正的 `.chanjet.com` 压底落位。
- **Keychain Secret Vault (保险柜)**：屏蔽开发层系统复杂性，利用 `go-keyring` 在 OS 深水位挂载私密明文保管匣；并在无主服务器以机器指纹作为盐做 AES 对称软加密退坡处理。
- **四维滚筒日志舱 (Rotary Logger Matrix)**：基于 Zap 的高性能纯粹格式化写出底座之上，引入 Lumberjack 根据设定的天数 (MaxAge) 与空间占用值 (MaxSize)，对 CLI 内产生的极其重要的系统运维、访问审计、流式凭证和死信这四大孤岛日志池实施纯物理文件分片轮转切割。

---

## 3. 单机隔离运行时物理级存储与区划 (Standalone Configuration Layout)

作为 `v0.1.1` 的唯一核心对齐红线，本期初始化阶段绝对屏蔽界接五花八门的投递尝试（不接入分布式中央件），**全力内聚构建只认本机的单兵物理沙盒保护环**。所有底层数据仓储与快照流一律转由切分明确、一目了然的用户挂载空间主导隔离（首推预设路径：`~/.chanjet-cli/`），实现三座互不交叉的数据“孤岛”：

```text
~/.chanjet-cli/
├── .config/                    # 无状态短效与租户环境收纳域
│   ├── config.yaml             # CLI 自身行为学公有参数（如 proxy默认端口）
│   ├── default.profile.yaml    # default 租户下的非敏环境脱密记录
│   └── cache/
│       └── vector_index.idx    # 单兵原生支持针对 Agent 用向量化固化的浮点矩阵缓存
├── db/                         # 内嵌底库阵列（事务性强存储保底池）
│   └── dlq.sqlite              # 未依赖外设数据库时的退化救起：采用纯 Go 编译期内嵌的零依赖本地 SQLite 代替
└── logs/                       # 文件分片轮转监控区（内置强约束的 Lumberjack 切割器，杜绝无限大体积）
    ├── system.log              # 【生命监测库】本地崩溃报警、后台服务宿主报错等系统态事件
    ├── audit.log               # 【全景审计池】客户端向本网关发出任意 API 时的快照验证留痕
    ├── stream.log              # 【消费防抵赖】通过 Webhook 收到的每一次远端全量帧落地快照
    └── dead-letter.log         # 【物理报警台】业务靶点失联、导致死信强行入库 `dlq.sqlite` 时触发的高亮联动红色告警
```
*(⚠️最高禁令注：基于分布式解耦的微服务中间件能力搭建预留，均已被彻底剥离且打包至顶级的 [《暂缓与未来演进特性池》](../../../shared/postponed_features.md) 中统一规划。本目录结构仅对单纯的单机系统全景退化负责。)*

---

## 4. 骨干业务时序流设计纲要 (Sequence Architectures)

### 4.1 强网防御加盾后的 API 执行时序 (Secured API Flow)
1. **沙盒接收指令**：用户/Agent 以命令 `cli api get /v1/user/123 --profile=A` 将流程交予总控引擎。
2. **提取状态临牌**：Profile Engine 借助密钥管理组件从 OS Keychain 中秘密捞取 `profile-A` 的加密指纹；如果该实例内的无风险 `openToken` 未过期则直取 `openToken`，过期则就地利用极光盾组件向认证服务器换取新短临口令。
3. **入参智能拼装**：通过 Trie-Tree 正反比对器匹配预读好的 OpenAPI AST，确认 `/v1/user/{id}` 模板合法，并填充参数 `123` 打散出标准的 Request 模型。
4. **硬核安检验身 (关键步骤)**：由被魔改后的 `http.Client` 拦截出网请求并压入 TLS：
   - 对方下发证书链，引擎即刻核发内置商业 CA 名单阵列的匹配探测。
   - 随之分解 `DNSNames` 不允许存在任何通配符的僭越。
   - 断言服务器叶子证书主域名后跟 `.chanjet.com` 血统防冒名。
5. **网关跃迁**：强验证一路亮绿灯，则附着 `openToken` 并拉响 mTLS (如果客户端指定) 将干净的数据发往开放平台核心。
6. **落盘与反馈**：从网线端拿回 API Json 载荷之后，向 `audit.log` 登记这一笔流水指纹。继而借由 Formatter 根据命令行的交互态势智能判断是打印高逼格 Table 表格还是干巴巴但容易解析的 Strict JSON。

### 4.2 Webhook 事件死信防丢时序 (DLQ Fallback Streaming)
1. **进程自燃隔离**：当命令行敲击启步开启服务器模式，利用组件自己 Fork 出一枚独立不关依赖 TTY 终端存活的 Daemon 守护真进程沉睡底层。SDK 开始与远端 Connector Server 交出长连接探针，静默听取全域。
2. **触发收包投递**：收到 JSON 体，Daemon 根据 `profile.yaml` 中的配置反查投送到何处（譬如：`http://localhost:8080/events-handler`）。
3. **多重退避防阻击**：
   - 若本地自己的应用返回 `200`，收工并在 `stream.log` 里按一下红章。
   - 只要本地应用挂了、断网或由于并发超载返 `503`/`500` 等错误，立即挂载 Exponential Backoff (指数后退器)：延缓等待 `1s->2s->4s->...->32s` 发起多次抢救心跳。
4. **落坟隔离告警**：若是重试弹匣全部打空，直接封存 Payload 核心帧压沉落入 `db/dlq.sqlite`；并同步向 `logs/dead-letter.log` 发送最高级 `[FATAL] Webhook Dispatch Dropped` 报警流。
5. **拯救与生还消费**：当内网小哥睡醒拔好网线恢复环境后，他只需输入 `cli webhook dlq-flush` 神操作，CLI 会像赶尸人般调动 SQLite 把沉睡队列捞出来以正确形态全推入靶机清空。
