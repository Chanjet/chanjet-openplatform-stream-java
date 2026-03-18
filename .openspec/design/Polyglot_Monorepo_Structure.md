# 畅捷通 Stream Gateway 多语言单仓 (Monorepo) 目录规划 v1.0

## 1. 总体目录树 (Global Directory Tree)

为了支持未来引入 Go、Rust 等服务，并将现有的 Java 实现平滑迁移至微服务集群架构，项目根目录结构调整如下：

```text
open-streaming-connector/
├── docs/                          # [共享] 业务需求与技术设计文档
│   ├── prd/                       # 产品需求文档 (v0.1.0, etc.)
│   └── design/                    # 详细技术设计文档 (Architecture, UML, etc.)
├── proto/                         # [核心契约] 跨语言协议定义 (Protobuf / gRPC / OpenAPI)
│   ├── internal/                  # 节点间 P2P 通讯协议 (.proto)
│   ├── gateway/                   # 外部接入协议定义 (.proto / .yaml)
│   └── model/                     # 共享数据模型定义
├── services/                      # [服务实现] 多语言微服务集群
│   ├── gateway-java/              # Java 实现的业务大脑与管理中心 (核心逻辑)
│   │   ├── connector-bom/
│   │   ├── connector-common/
│   │   ├── connector-api/
│   │   ├── connector-core/
│   │   ├── connector-infra/
│   │   └── connector-server/
│   ├── gateway-go-receiver/       # [未来] Go 实现的高并发 HTTP Webhook 接收器
│   └── gateway-rust-pusher/       # [未来] Rust 实现的高性能 WebSocket 推送器
├── sdk/                           # [多语言 SDK] 供 ISV 使用的接入库
│   ├── java/                      # Java SDK
│   ├── python/                    # Python SDK
│   ├── go/                        # Go SDK
│   └── rust/                      # Rust SDK
├── infra/                         # [基础设施] 共享部署与运维脚本
│   ├── docker/                    # Dockerfile 与 Docker Compose
│   ├── k8s/                       # Kubernetes Deployment 资源定义
│   └── terraform/                 # 基础设施即代码 (IaC)
├── scripts/                       # [构建辅助] 统一的 CI/CD 脚本与 Makefile
└── Makefile                       # [入口] 统一的构建、测试与启动指令
```

---

## 2. 核心目录职责说明

### 2.1 `proto/` (契约驱动的核心)
**职责**：存放所有跨语言通讯的 IDL (接口定义语言) 文件。
- **作用**：Java、Go、Rust 服务必须根据这里的 `.proto` 文件通过编译器（如 `protoc`）生成各自的领域模型代码。
- **演进**：将 `connector-common` 中的 Java Records 逻辑上移到此目录，作为所有语言的共同祖先。

### 2.2 `services/` (多语言异构服务)
**职责**：每个子目录代表一个独立的进程或微服务。
- **`gateway-java/`**: 现有的 Java 业务逻辑。它将通过 `api` 契约定义与 `infra` 实现进行解耦，方便未来将其中的高并发 I/O 模块替换为其他语言实现。
- **松耦合体现**：Java 服务作为逻辑中心，负责处理复杂的 30 分钟容忍期状态机。

### 2.3 `sdk/` (跨语言客户端)
**职责**：统一存放所有提供给 ISV 的客户端库。
- **收益**：ISV 在使用 Python 或 Java 开发时，看到的协议字段、心跳间隔、退避算法将在这里通过共享测试用例得到最终一致性保证。

### 2.4 `infra/` (共享基础设施)
**职责**：管理 Redis Cluster、LB (负载均衡) 的配置，以及多语言容器的编排。
- **一致性**：无论服务是用什么语言写的，其健康检查逻辑、日志格式要求、监控指标（Prometheus）都在这里统一规范。

---

## 3. 多语言演进路径建议

1.  **Phase 1 (Java Baseline)**:
    按照目前的规划在 `services/gateway-java/` 中完成全功能开发，确保业务逻辑闭环。
2.  **Phase 2 (Proto Transition)**:
    将 `gateway-java` 中的 POJO 定义提取为 `proto/` 目录下的 Protobuf 定义，Java 服务切换为自动生成的代码。
3.  **Phase 3 (I/O Refactor)**:
    如果 Webhook 接收压测遇到瓶颈，在 `services/gateway-go-receiver/` 中使用 Go 重写 HTTP 入口，并通过 Redis 路由与 Java 逻辑层互通。
4.  **Phase 4 (Connection Refactor)**:
    如果数万个 WS 连接导致 Java GC 频繁，在 `services/gateway-rust-pusher/` 中使用 Rust 重写 WS 连接层，提升内存利用率。

---

## 4. 协作开发契约

- **变更申请**：任何涉及协议字段的修改（如 `EventFrame` 增加字段），必须先修改 `proto/` 下的定义，经由多语言团队评审通过。
- **构建入口**：统一使用根目录下的 `Makefile`。例如 `make build` 应自动识别子目录下的 `mvn`、`go build` 或 `cargo build`。
