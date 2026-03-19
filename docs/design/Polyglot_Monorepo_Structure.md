# 畅捷通 Stream Gateway 多语言单仓 (Monorepo) 目录规划 v1.0

## 1. 总体目录树 (Global Directory Tree)

项目采用 Monorepo 模式组织，初期以 Java 21 实现为核心，同时预留多语言扩展空间。

```text
open-streaming-connector/
├── docs/                          # [共享] 业务需求与技术设计文档
├── proto/                         # [核心契约] 跨语言协议定义 (Protobuf)
│   └── model/                     # 共享帧格式 (.proto)
├── services/                      # [服务端实现] 
│   └── gateway-java/              # Java 21 (Virtual Threads) 实现的核心网关
├── sdk/                           # [多语言 SDK] 供 ISV 接入
│   ├── java/                      # Java SDK (已完成)
│   └── python/                    # Python SDK (规划中)
├── infra/                         # [基础设施] 部署模板
├── scripts/                       # [辅助工具] 验证脚本与工具
├── .mvn/                          # 项目特定 Maven 配置 (含内网 settings)
└── Makefile                       # [统一入口] 构建与测试指令
```

---

## 2. 核心目录职责说明

### 2.1 `proto/` (契约驱动)
**职责**：存放所有跨端通讯的 IDL 文件。
- **作用**: 确保 SDK 与服务端对 `EventFrame` 的二进制/JSON 序列化理解一致。

### 2.2 `services/gateway-java/` (逻辑中心)
**职责**：网关的完整实现。
- **高性能**: 利用 Java 21 虚拟线程特性，单进程即可承载数万连接，降低了引入 Rust/Go 优化的迫切性。
- **模块化**: 内部划分为 API, Core, Infra, Server 四个层级，支持未来的局部替换。

### 2.3 `sdk/` (跨语言客户端)
**职责**：提供给 ISV 的开发包。
- **一致性**: 所有语言 SDK 必须遵循相同的指数退避重连算法和安全签名逻辑。

---

## 3. 多语言演进现状

1.  **Java Baseline (v0.1.0)**: 已完成。基于虚拟线程的 Java 实现已达到极高的吞吐性能。
2.  **SDK Expansion**: 接下来将重点补充 Python SDK，以满足不同 ISV 的技术栈需求。
3.  **Containerization**: 相关的 Dockerfile 与 K8s 部署脚本已从 Makefile 剥离，转由专门的 DevOps 流程管理。

---
**更新日期**: 2026-03-19
