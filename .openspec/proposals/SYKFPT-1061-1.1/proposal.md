# OpenSpec 提案：Monorepo 骨架搭设 (SYKFPT-1061-1.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-1.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 1.1: Monorepo 骨架搭设 |

---

## 1. 问题背景 (Context)
本项目定位为一个高性能、低延迟的 Webhook-to-WebSocket 同步桥接器。为了支持未来 Java、Go、Rust 等多种语言实现的微服务协作，并共享协议契约（Proto）与 SDK，我们需要建立一个 **Polyglot Monorepo (多语言单仓)** 结构。

## 2. 目标 (Objectives)
- 建立统一的目录层次结构。
- 确立跨语言协议（Proto）的存放位置。
- 提供根目录级别的构建入口（Makefile）。
- 为现有的 Java 开发流预留 `services/gateway-java` 目录。

## 3. 技术设计 (Technical Design)

### 3.1 目录结构规划
```text
.
├── Makefile                       # 统一构建入口
├── proto/                         # 跨语言 IDL (Protobuf)
│   ├── internal/                  # 内部 P2P 协议
│   └── gateway/                   # 外部接口定义
├── services/                      # 微服务集群
│   └── gateway-java/              # Java 实现的主模块
├── sdk/                           # 多语言接入库
│   ├── java/
│   └── python/
├── infra/                         # 共享基础设施 (Docker/K8s)
├── docs/                          # 设计与产品文档 (已存在)
└── scripts/                       # 辅助脚本
```

### 3.2 根目录 Makefile 设计
提供以下核心指令：
- `make init`: 初始化开发环境。
- `make build-java`: 构建 Java 服务模块。
- `make test`: 执行集成测试。
- `make clean`: 清理各模块构建产物。

## 4. 实施计划 (Implementation Plan)
1.  **创建目录结构**：使用 `mkdir -p` 创建上述规划中的所有空目录。
2.  **编写根目录 Makefile**：实现基本的构建骨架。
3.  **迁移文档**：确保 `docs/` 目录位于根目录下。
4.  **初始化 Gitignore**：配置全局的 `.gitignore`。

## 5. 验证策略 (Verification Strategy)
- **结构验证**：检查所有预定义的目录是否已正确创建。
- **构建验证**：运行 `make build-java`（初始为空）确保 Makefile 逻辑连通。
- **Git 状态验证**：确认所有变更已正确纳入版本控制。

---
**审批意见**：待评审。
