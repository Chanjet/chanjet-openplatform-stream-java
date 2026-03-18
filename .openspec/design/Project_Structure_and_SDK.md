# 畅捷通 Stream Gateway 项目结构与 SDK 设计 v1.1.0

## 1. 架构演进：从单仓到多语言单仓 (Monorepo)

本项目采用 **Polyglot Monorepo** 结构，以支持未来 Java、Go、Rust 的多语言微服务协作。

### 1.1 核心目录布局

| 目录 | 职责内容 | 协作方式 |
| :--- | :--- | :--- |
| **`proto/`** | 跨语言协议契约 (.proto / OpenAPI) | **单源真相**。所有语言通过编译器生成代码。 |
| **`services/`** | 多语言微服务集群实现 | 独立进程，通过 P2P 协议或 Redis 路由互通。 |
| **`sdk/`** | 面向 ISV 的多语言接入库 | 逻辑对齐，独立发布。 |
| **`docs/`** | 统一的设计与产品需求文档 | 全局可见。 |

---

## 2. Java 模块内部结构 (services/gateway-java)

在 `services/gateway-java/` 目录下，继续沿用 Maven 多模块结构，并遵循严格的松耦合设计。

| 模块名称 | 职责定位 | 核心依赖 |
| :--- | :--- | :--- |
| **`connector-bom`** | 依赖版本管理 | 无 |
| **`connector-common`** | 协议模型 (逐步迁移至 proto 生成) | 生成的代码, Jackson |
| **`connector-api`** | **SPI 接口契约** (IRouteStore, IConnectionManager) | 无 |
| **`connector-core`** | **领域逻辑实现** (状态机、路由寻址) | `connector-api` |
| **`connector-infra`** | **适配实现** (Redis, Core Client) | `connector-api`, Redis |
| **`connector-server`** | **传输适配与启动** (Spring Boot 4) | `connector-core`, `connector-infra` |

---

## 3. 设计原则：跨语言解耦

### 3.1 协议驱动开发 (Protocol-Driven)
- **不再**以 Java 类定义作为协议标准。
- **改为**以 `proto/` 目录下的 IDL 文件作为唯一标准。
- 收益：Java 端的 `EventFrame` Record 和 Go 端的 `EventFrame` Struct 具有完全一致的序列化结果。

### 3.2 共享 Redis 状态空间
- 所有的网关节点（无论 Java/Go/Rust 实现）均遵循 `docs/design/Data_Model_and_State.md` 中定义的 Redis Key 规范。
- 这样，Go 服务接收到的 Webhook 即使目标连接在 Rust 服务上，也可以通过共享路由表找到目标 Node 并通过 P2P 转发。

### 3.3 统一构建流水线
- 根目录提供 `Makefile` 封装各语言构建细节。

---

## 4. 并行开发协作工作流 (Parallel Development)

在契约固定的前提下，系统支持五个开发流并行推进，最大限度缩短交付周期。

### 4.1 并行开发流划分
1. **契约组 (Team Foundation)**：维护 `proto/` 和 `connector-api`。交付点为 Interface 定义和多语言 Generated Code。
2. **逻辑组 (Team Core)**：负责 `connector-core`。通过 Mockito 模拟 API 接口，先行实现路由分发、30分钟容忍期及熔断背压逻辑。
3. **实现组 (Team Infra)**：负责 `connector-infra`。利用 TestContainers 独立验证 Redis 路由和 Core Client 适配层，不依赖业务逻辑。
4. **接入组 (Team Server)**：负责 `connector-server`。专注于协议升级、会话维护及 Java 21 虚拟线程配置。
5. **SDK组 (Team SDK)**：负责多语言 SDK。基于 `proto` 定义和 Mock Server 独立开发，确保 ISV 接入体验一致。

### 4.2 隔离开发技术手段
- **API Mocks**：逻辑组在单元测试中使用 Mockito 实现与存储层的解耦。
- **TCK (Technology Compatibility Kit)**：定义一套跨语言消息流转测试集，作为各服务集成后的验收标准。
- **Contract-First Testing**：SDK 组利用 Mock Server 模拟后端响应，实现后端开发零阻塞。

---

## 5. 目录结构树预览

```text
open-streaming-connector/
├── proto/                         # 跨语言 IDL
├── services/                      # 微服务集群
│   └── gateway-java/              # Java 实现
│       ├── connector-api/
│       ├── connector-core/
│       └── ...
├── sdk/                           # 多语言 SDK
│   ├── java/
│   └── python/
└── Makefile                       # 构建入口
```
