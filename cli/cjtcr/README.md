# 畅捷通 Stream Connector CLI (Rust 版) - cjtcr

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`cjtcr` 是畅捷通开放平台（Chanjet Open Platform）官方推出的高性能、安全且智能的命令行治理工具。它旨在取代原有的 Go 版本，通过 Rust 的内存安全特性、零成本抽象以及原生 AI 能力，为开发者提供全生命周期的 API 治理体验。

## ✨ 核心特性

- 🧠 **语义化 API 搜索**：集成原生 ONNX 引擎（BGE-small-zh），支持基于 NLP 的意图发现，不仅仅是关键词匹配。
- 🛡️ **声明式安全治理**：基于 OpenAPI 规约自动注入 `appKey`、`appSecret` 以及 `openToken`，实现零代码侵入的生产级鉴权。
- 📦 **多环境 Profile 隔离**：原生支持 `default`、`inte`、`prod` 等多套环境一键切换，配置与凭据物理隔离。
- 🚀 **高性能异步架构**：基于 `tokio` 实现的异步 IO 架构，在处理高并发 Stream 桥接与大规模 API 扫描时表现卓越。
- 📋 **生产级遙测系统**：支持多域（sys, audit, stream, dlq）结构化日志，具备自动切片、滚动保存与实时 `log --follow` 追踪能力。

## 🚀 快速开始

### 1. 构建与安装

```bash
cd cli/cjtcr
make build
# 安装二进制到系统路径 (可选)
cp ../../bin/cjtc /usr/local/bin/
```

### 2. 初始化环境 (必须包含 AppKey, Secret 与 Certificate)

```bash
cjtc init --app-key YOUR_KEY --app-secret YOUR_SECRET -c YOUR_CERT
```
*(注：`--webhook-target` 等参数为可选，可在初始化后通过 `config` 查看或修改)*

### 3. API 探索与调用

```bash
# 语义搜索接口
cjtc api list -s "创建订单"

# 调用接口 (自动注入鉴权头)
cjtc api POST /v1/orders/create -d '{"amount": 100}'
```

### 4. 开启本地代理与 Stream 桥接

```bash
cjtc daemon start
```

## 📖 详细文档

- [架构设计 (Architecture)](docs/ARCHITECTURE.md) - 深入了解 `RequestDecorator`、插件系统与安全性设计。
- [命令指南 (Commands)](docs/COMMANDS.md) - 全量的子命令参数说明（Api, Auth, Dlq, Log, Profile）。
- [日志与遥测 (Logging)](docs/LOGGING.md) - 如何配置日志滚动、清理策略与结构化审计。

## 🛠️ 开发者指南

本项目遵循严格的 **TDD (测试驱动开发)** 流程。所有功能实现必须包含相应的测试用例。

### 常用开发命令

```bash
# 运行单元测试
make test
# 构建并运行测试版 (使用 .cjtc-test 配置目录)
make build-test
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
