# 畅捷通 Stream Connector CLI (Rust 版) - cowen

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.5.0-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`cowen` 是畅捷通开放平台（Chanjet Open Platform）官方推出的高性能、安全且智能的 API 治理与侧车 (Sidecar) 代理底座。基于严谨的 Crate 分层架构与 Thin CLI 交互设计，它深度融合了 WebAssembly 插件隔离、纯 Rust 原生独立 AI 检索引擎与云原生存储底座，为各类终端与 SaaS 应用提供极简、高扩展的全生命周期接入体验。

## ✨ 核心特性

- 🖥️ **Thin CLI 瘦客户端架构**：将所有的状态机、接口代理与鉴权逻辑下沉至 `cowen-daemon` 守护进程，CLI 完全降级为无状态的轻量级 IPC 客户端，根除了多端并发调用的竞态问题。
- 🧠 **独立 AI 语义检索 (Standalone Sidecar)**：彻底剥离内嵌的厚重 ONNX 引擎，重构为 100% 纯 Rust 实现的独立搜索 Sidecar。支持严格的租户级隔离 (Tenant Namespace) 以及 < 5ms 热启动的极速磁盘级向量缓存。
- 🧩 **Wasm 插件扩展与沙盒化隔离**：核心代理网关融合了 Phase 3 标准的 Wasmtime 引擎，同时提供了跨平台无特权 (Unprivileged) 操作系统原生沙盒环境，供第三方可执行二进制插件安全运行与命令拦截。
- 🗄️ **多模态可插拔存储底座**：彻底解耦配置层与持久化驱动，原生支持基于本地 SQLite 的单机运行，或接入远端 Redis Lua CAS 驱动实现 Serverless / K8s 多副本容灾共享。
- 🔄 **动态 OpenAPI 发现与声明式治理**：通过双向 Stream 信道自动同步远端规约与可访问列表；实现 `appTicket` 与 `openToken` 等多维凭证的自动捕获与零代码侵入注入。
- 🤖 **MCP (模型上下文协议) 原生支持**：内置 MCP 协议接入标准，可无缝对接主流大模型智能体与 AI Agents（如 Cursor、Claude 等），打通自然语言驱动的本地能力编排。
- 📉 **指数退避限流与生产级遥测**：内置云端流量防击穿算法（基于持久化存储的 Backoff），配有多域 (Sys/Audit/Stream/DLQ) 高性能结构化日志追踪。

## 🚀 快速开始

### 1. 构建与装包

使用 `Makefile` 进行跨平台构建，编译与打包产物统一输出至 `../../bin/` 目录下，并自动完成二进制插件的鉴权签名与包级 MD5/SHA1 完整性校验。

```bash
cd cli/cowen
# 构建 macOS 平台产物并生成 .pkg 组件级安装包
make package-macos-aarch64
# 构建 Linux 平台产物 (利用 Podman 隔离编译以支持旧版本 glibc)
make linux-x86_64-with-podman
# 构建 Windows 平台原生产物或执行交叉编译
make windows-x86_64
make windows-x86_64-cross
# 安装至当前系统路径
make install
```

### 2. 初始化环境

```bash
cowen init --app-mode self_built --app-key YOUR_KEY --app-secret YOUR_SECRET -c YOUR_CERT
```

### 3. API 探索与调用

```bash
# 语义搜索接口
cowen api list -s "创建订单"

# 强制刷新动态规约缓存
cowen api list --refresh

# 调用接口 (自动注入鉴权头)
cowen api POST /v1/orders/create -d '{"amount": 100}'
```

### 4. 开启本地代理与 Stream 桥接

```bash
cowen daemon start --proxy-port 8080
```

## 📚 架构与模块导读 (Architecture & Modules)

在 `0.5.0` 版本中，Cowen 采用严格的分层重构（Thin CLI + Daemon + Sidecar Plugins）。以下是核心 Crate 矩阵，点击链接查看各自的详细职能边界：

### 1. App 接入层 (`crates/app`)
*Thin CLI 交互前端与守护进程入口*
- [`cowen-cli`](crates/app/cowen-cli/README.md) - 轻量级客户端终端程序，无状态的 IPC 发起者。
- [`cowen-daemon`](crates/app/cowen-daemon/README.md) - 常驻后端的守护进程，负责加载服务和治理插件。
- [`cowen-server`](crates/app/cowen-server/README.md) - 统筹网络监听与本地 Proxy 转发。

### 2. Adapters 适配层 (`crates/adapters`)
*协议转换与沙箱入口点*
- [`cowen-grpc-facade`](crates/adapters/cowen-grpc-facade/README.md) - 统一处理 gRPC 和 IPC 端点定义与模型适配。
- [`cowen-wasm-facade`](crates/adapters/cowen-wasm-facade/README.md) - 负责 Wasmtime 虚拟机实例化和宿主函数注入。

### 3. Services 领域服务层 (`crates/services`)
*解耦后的核心微服务实现*
- [`cowen-auth`](crates/services/cowen-auth/README.md) - OAuth2/自建应用 Token 生命周期治理。
- [`cowen-config`](crates/services/cowen-config/README.md) - 多源环境配置加载与订阅机制。
- [`cowen-doctor`](crates/services/cowen-doctor/README.md) - 系统环境体检与网络连通性诊断。
- [`cowen-monitor`](crates/services/cowen-monitor/README.md) - 遥测数据、健康埋点及 Audit 日志系统。
- [`cowen-search`](crates/services/cowen-search/README.md) - 全局语义搜索分发与 RAG 上下文拼装。
- [`cowen-store`](crates/services/cowen-store/README.md) - 屏蔽 SQLite 与 Redis 的多态数据持久化实现。

### 4. Core 核心底座 (`crates/core`)
*跨平台基建与接口契约*
- [`cowen-capabilities`](crates/core/cowen-capabilities/README.md) - 最高级 Trait 契约，解耦所有强依赖关系的抽象池。
- [`cowen-common`](crates/core/cowen-common/README.md) - 错误枚举、全局常量及共享模型。
- [`cowen-gateway`](crates/core/cowen-gateway/README.md) - 负责零信任应用网关（Identity-Aware Gateway）的路由匹配、会话校验与凭证 wash。
- [`cowen-infra`](crates/core/cowen-infra/README.md) - 底层网络组件、HTTP 客户端池与并发队列编排。
- [`cowen-macros`](crates/core/cowen-macros/README.md) - 内部专用的元编程及编译期校验宏集合。
- [`cowen-plugin`](crates/core/cowen-plugin/README.md) - 侧车 (Sidecar) 进程与标准流 IPC 生命周期引擎。
- [`cowen-sys`](crates/core/cowen-sys/README.md) - 负责跨平台 OS 抽象（如锁、IPC、文件系统挂载）。

### 5. Plugins 官方插件层 (`crates/plugins`)
*独立分发的运行态外延组件*
- [`cowen-mcp-plugin`](crates/plugins/cowen-mcp-plugin/README.md) - 面向外部 AI Agent 开发的 Model Context Protocol 独立侧车。
- [`cowen-search-embedding`](crates/plugins/cowen-search-embedding/README.md) - 原生 Rust 实现的高性能向量与检索侧车。
- [`cowen-wasm-auth-selfbuilt`](crates/plugins/cowen-wasm-auth-selfbuilt/README.md) - `Self-Built` 模式专供鉴权沙盒 Wasm。
- [`cowen-wasm-auth-storeapp`](crates/plugins/cowen-wasm-auth-storeapp/README.md) - `Store-App` 模式专供鉴权沙盒 Wasm。

### 6. Tools 构建工具层 (`crates/tools`)
*本地或 CI/CD 开发辅助*
- [`cowen-signer`](crates/tools/cowen-signer/README.md) - 为侧车及 Wasm 计算防篡改哈希和数字签名的构建工具。

## 📖 详细文档

- [架构设计 (Architecture)](docs/ARCHITECTURE.md) - 深入了解 `AuthProvider` SPI、插件系统与安全性设计。
- [技术规范 (Technical Spec)](docs/TECHNICAL_SPEC.md) - 详述存储驱动、Token 维护生命周期与自愈逻辑。
- 🚀 **快速上手**:
    - [Self-Built 模式指南](docs/usage/self_built.md)
    - [Store-App 模式指南](docs/usage/store_app.md)
    - [OAuth2 模式指南](docs/usage/oauth2.md)
- 🛠️ **进阶运维**:
    - [进阶运维与自愈指南 (DLQ/诊断/集群)](docs/usage/OPERATIONS.md)
- [命令指南 (Commands)](docs/COMMANDS.md) - 全量的子命令参数说明（Api, Auth, Daemon, Store, System）。
- [日志指南 (Logging)](docs/LOGGING.md) - 详述多域遥测系统与日志滚动策略。
- [历史文档归档 (Archive)](docs/archive/v1/) - 包含 owenc 时期的历史规范与设计初稿。

## 🛠️ 开发者指南

本项目遵循严格的 **TDD (测试驱动开发)** 流程。

### 常用开发命令

```bash
# 运行单元测试
cargo test
# 运行集成测试套件 (E2E)
make test
# 运行命名规范验证测试
bash tests/makefile_test.sh
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
