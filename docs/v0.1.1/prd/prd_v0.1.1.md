# 畅捷通 Openplatform Stream Connector CLI - 产品需求文档 (PRD v0.1.1)

> 本文档融合了前期探讨的《需求文档草案 (draft)》及《技术可行性分析文档》。从工具产品的功能定义延伸至底层工程的容错与技术栈考量，是该 v0.1.1 版本研发落地及验收的唯一基准指导。

---

## 1. 产品定位与核心生态
**定位**：一个基于 **Go (Golang)** 研发的支持后台常驻运行的跨平台命令行工具（CLI）。它兼具**畅捷通开放平台 API 无感调用**、**流式事件监听及 Webhook 转发**、以及**本地透明 HTTP 代理**的三重网关能力。
**第一视角生态 (Agent-First)**：本产品在设计上不仅面向普罗开发者，更致力于成为异构大模型（如 openClaw 等自主 Agent）的御用底层执行引擎。为此，所有的输出都必须严格遵守机器友好的规范结构，并同步产出专属的服务端 **AI Agent Skill (指令剧本)** 作为衍生交付物。

---

## 2. 工程架构与技术底线约束
结合性能与 Agent 适应性，研发过程需严格遵循以下底层技术选型及底线设计：
1. **纯 Go 与交叉编译体系**：全量代码必须优先采用 Pure Go 组件（例如放弃依赖 `go-sqlite3` 转用纯 Go 实现，摒弃重型向量库 C++ 依赖）。确保开发机可通过设置 `GOOS` 和 `GOARCH` 零痛点直出适用于 Mac、Linux 与 Windows 的单执行静态二进制文件。
2. **绝对非交互防死锁机制**：禁止在所有核心路径（如写入凭证）强制使用阻塞式人机提示（Prompts）。所有功能 100% 被 `--flag`、环境变量（ENV）或标准输入流覆盖，保证在被 Agent 后台静默调用时永不死锁挂起。
3. **结构化的 JSON/YAML `STDOUT`**：全局提供并严格尊重 `--format json` 与 `--format yaml` 输出选项，无论执行查表、状态读取亦或是 Throw Exception。一旦在命令中声明，任何彩色 UI、进度条等人类观感元素均需被底层彻底抹平至纯粹严谨的 JSON 或 YAML 格式数据回吐模式；同时对于异常信息须原生自带纠偏建议（Suggestion），以便大模型进行自我修正重试。
4. **多实例安全托管**：密码本及长效刷新所需的 `appSecret` 绝不使用明文落盘。多租户敏感数据需引入 `Go-Keyring` 等组件挂载操作系统底层的秘钥保险箱（Keychain / Credential Manager）；当在无界面服务器下时则采用动态加盐对配置文件执行 AES-GCM-256 对称加密。
5. **纯粹原生物理系统极简闭环 (Standalone Fallback Isolation)**：对于 `v0.1.1` 首发期，严守克制边界底线，严禁在产品中盲目横向引入并联接任何繁重型的远端外设中间件组网（如 Redis / DB 等集群依赖）。工具核心状态机运作的全部数据流量矩阵（含核心极敏 Token、向量检索高速矩阵倒排以及容灾防丢的死信账本）一律且只能就地收束进跨平台的操作系统原生级本地保险箱（OS Keyring）及无依赖自举的轻量型（纯 Go SQLite）结构树。以此来构筑一个像 U 盘般“干拔无损、零前置运维开销、开箱即拉全战备”的核心产品护城河。(⚠️关于面临企业级高可用编排下的微服务泛化存储解耦扩容能力与多实例强同步并发挑战，已全部归档至顶级的 [延缓与未来演进特性池](../../shared/postponed_features.md) 暂不做处理)。

---

## 3. 功能模块与交互全景 (Commands)

> 💡 **全局规范**：
> - **多实例别名 `--profile`**：所有命令默认对首个初始化的 `default` 实例生效。一旦用户挂载 `--profile shop-A` 参数，该命令将自动切入独立加密的上下文空间寻找执行锚点。
> - **全景帮助手册 `--help`**：根目录输入 `cli --help` 将结构化打印全部子命令大纲及全局参数的系统能力透视。对具体的末端节点（如 `cli api get /v1/orders --help`）亦可动态拉取该 API 独占的入参级别明细说明及字段解释。

### 3.1 生命周期与凭据挂载 (`init`, `reset`, `config`)
- **首次建联引导 (`init`)**：
  - **前置建站确认**：优先提醒用户“是否已于开放平台后台创建自建应用？”。对于无头苍蝇的新手（选择“否”），直接在本地打印内含外链的 `创建与取参新手指南`。
  - **应用模式定死与凭据挂载 (App Mode)**：待确认无误后，系统要求注入挂载参数。**当前 v0.1.1 版本仅支持唯一、默认的「自建应用 (self-built)」模式**。需注入 `appKey`、`appSecret` 及自建证书。
  - **安全加固 (Vault)**：敏感凭据（AppSecret、Certificate、EncryptKey）由内置 Vault 托管，并绑定机器指纹（Machine Fingerprint）进行本地加密存储，严防配置文件被直接拷贝冒用。
- **状态归零 (`reset`)**：物理清除本地当前 `profile` 的令牌状态机及安全缓存区，强行切回空白状态。
- **冷启动寻票保活 (Ticket Bootstrap)**：对于需要强依赖 `appTicket` 的链路，当系统服务（Daemon）启动或重启时，若发现本地尚未持有或持有的 `appTicket` 已过期，引擎立即主动触发推送。
- **配置清查 (`config`)**：查看当前 Profile 下的非敏感配置详情（如 URL、Webhook 目标、应用模式等）。

### 3.2 动态 API 调用体验 (`api`)
为了摒弃 OpenAPI 中残缺不全的 `tags` 或 `operationId` 字段，采用了 **`Method + Path`** 直觉执行范式（即 `{cjtc} api <METHOD> <PATH> [flags]`）。

- **智能路径拼装与无感调用**：
  提供 `api [METHOD] [PATH]` 模式。CLI 自动从 Vault 检索 Token 并注入 `Authorization` 头，支持通过 `-d` 指定 JSON 数据（可多次指定以拼接长 JSON）。
- **文档查看与示例生成 (`spec`)**：
  新增 `api spec [METHOD] [PATH]` 子命令。除展示接口摘要、参数列表与 Response Schema 外，还具备 **智能示例生成** 能力，根据接口定义自动生成可直接运行的 `cjtc api ...` 调用指令。
- **防幻觉试运行机制 (Dry-Run)**：支持附加 `--dry-run`；仅基于存储的 Schema 做本地校验而不发起真实网络调用。
- **内嵌语义检索 (Semantic Search/list)**：
  支持 `list --search`。基于本地嵌入（Embedding）索引实现。
  - **参数控制**：通过 `--top` (或 `-n`) 控制返回结果数量，默认返回前 5 条最相关的 API。
  - **资源占用**：采用轻量级向量检索，索引文件存放于 `~/.cjtc/{profile}_openapi.idx`。

### 3.3 无服务网关边界的延展 (`webhook`, `proxy`, `daemon`)
- **守护进程管理 (`daemon`)**：
  - **后台运行**：支持 `daemon start -d` 以后台模式启动，并将 PID 记录于 `~/.cjtc/daemon.pid`。
  - **优雅停机**：提供 `daemon stop` 命令，通过向进程发送 `SIGTERM` 信号实现优雅关机与资源释放。
  - **本地代理**：默认监听 `127.0.0.1:8080`，提供免鉴权代理能力，自动为本地请求附加安全凭据。
- **Connector Stream 流式消费 (`webhook`)**：
  - **死信队列 (DLQ)**：通过 `webhook dlq list` 查看因目标宕机而积压的消息，并通过 `webhook dlq retry <id>` 执行手动补偿重试。

### 3.4 可观测性治理体系 (`log`, `status`, `check-update`)
- **日志的四层塔治理 (`log`)**：
  - **文件布局**：日志存储于 `~/.cjtc/log/{domain}.log`。
  - **实时查看 (`view`)**：支持 `log view -d [domain] -n [lines]` 实时阅览，提供针对 `audit` 域的彩色结构化美化输出。
- **深度诊断视图 (`status`)**：
  - **全局概览**：默认（不指定 `--profile`）展示所有已初始化环境的 AUTH 状态与 DAEMON 运行状态。
  - **详细诊断**：指定 `--profile` 时，输出详细的诊断报告，包括 AppTicket 持有情况、AccessToken 有效性、Daemon PID 及其监听端口。
  - **纠偏建议 (Next Steps)**：根据诊断结果（如 `AWAITING_TICKET` 或 `NOT_RUNNING`）自动提供修正指令建议。

---

## 5. 本地文件系统拓扑 (Appendix)
工具的核心数据收束于用户家目录下的 `.cjtc/` 文件夹：
- `{profile}.yaml`: 环境基础配置。
- `.seal`: Vault 安全存储索引。
- `daemon.pid`: 运行中的后台进程标识。
- `{profile}_openapi.json`: 本地缓存的 OpenAPI 规范。
- `{profile}_openapi.idx`: 语义搜索向量索引。
- `log/`: 包含 `audit.log`, `sys.log`, `stream.log`, `dlq.log`。
- `dlq.db`: 死信队列持久化存储。


---

## 4. 交付清单
1. 包含针对各 CPU/OS 等架构的单体二进制产物（以 GoReleaser 发版管理形式落地）。
2. 在工程源码下维护且随时同步演进的 **AI Agent Skill Markdown 插件规范**。供使用者导入自己的私有大模型/助手（如 openClaw）中，赋能机器一键实现对该网关 CLI 的自我学习部署调优和自然语言操作对话流。
