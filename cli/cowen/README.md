# 畅捷通 Stream Connector CLI (Rust 版) - cowen

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.6-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`cowen` 是畅捷通开放平台（Chanjet Open Platform）官方推出的高性能、安全且智能的命令行治理工具。它通过 Rust 的内存安全特性、零成本抽象以及原生 AI 能力，为开发者提供全生命周期的 API 治理体验。

## ✨ 核心特性

- 🧠 **语义化 API 搜索**：集成原生 ONNX 引擎（BGE-small-zh），支持基于 NLP 的意图发现，不仅仅是关键词匹配。
- 🔄 **动态 OpenAPI 发现**：服务驱动的 Spec 同步机制，支持全量规约下载与增量权限发现，自动聚合环境感知的可用接口列表。
- 🛡️ **声明式安全治理**：基于 OpenAPI 规约自动注入 `appKey`、`appSecret` 以及 `openToken`，实现零代码侵入的生产级鉴权。
- 📉 **指数退避与限流 (Backoff)**：内置针对云端流量控制（HTTP 409）的指数级退避算法，状态加密持久化，保障在高频调用或自愈重启时的账号信誉。
- 📦 **多环境 Profile 隔离**：原生支持 `default`、`inte`、`prod` 等多套环境一键切换，配置与凭据物理隔离。
- 🚀 **高性能异步架构**：基于 `tokio` 实现的异步 IO 架构，在处理高并发 Stream 桥接与大规模 API 扫描时表现卓越。
- 📋 **生产级遙测系统**：支持多域（sys, audit, stream, dlq）结构化日志，具备自动切片、滚动保存与实时 `log --follow` 追踪能力。

## 🚀 快速开始

### 1. 构建与安装

使用 `Makefile` 进行跨平台构建，产物位于 `bin/` 目录下，并自动附带 MD5/SHA1 校验文件。

```bash
cd cli/cowen
# 构建当前平台二进制 (Full Version)
make windows-x86_64
# 构建兼容旧款 CPU (无 AVX) 的版本
make windows-x86_64-legacy
# Linux 兼容版本 (使用 Podman)
make linux-x86_64-legacy-with-podman
# 安装到系统路径
make install
```

### 2. 初始化环境

```bash
cowen init --app-key YOUR_KEY --app-secret YOUR_SECRET -c YOUR_CERT
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
