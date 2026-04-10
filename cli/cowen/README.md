# 畅捷通 Stream Connector CLI (Rust 版) - cowen

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.5-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`cowen` 是畅捷通开放平台（Chanjet Open Platform）官方推出的高性能、安全且智能的命令行治理工具。它通过 Rust 的内存安全特性、零成本抽象以及原生 AI 能力，为开发者提供全生命周期的 API 治理体验。

## ✨ 核心特性

- 🧠 **语义化 API 搜索**：集成原生 ONNX 引擎（BGE-small-zh），支持基于 NLP 的意图发现，不仅仅是关键词匹配。
- 🔄 **动态 OpenAPI 发现**：服务驱动的 Spec 同步机制，支持全量规约下载与增量权限发现，自动聚合环境感知的可用接口列表。
- 🛡️ **声明式安全治理**：基于 OpenAPI 规约自动注入 `appKey`、`appSecret` 以及 `openToken`，实现零代码侵入的生产级鉴权。
- 📦 **多环境 Profile 隔离**：原生支持 `default`、`inte`、`prod` 等多套环境一键切换，配置与凭据物理隔离。
- 🚀 **高性能异步架构**：基于 `tokio` 实现的异步 IO 架构，在处理高并发 Stream 桥接与大规模 API 扫描时表现卓越。
- 📋 **生产级遙测系统**：支持多域（sys, audit, stream, dlq）结构化日志，具备自动切片、滚动保存与实时 `log --follow` 追踪能力。

## 🚀 快速开始

### 1. 构建与安装

使用 `Makefile` 进行跨平台构建，产物位于 `bin/` 目录下，并自动附带 MD5/SHA1 校验文件。

```bash
cd cli/cowen
# 构建当前平台二进制
make macos-aarch64    # 或 linux-x86_64, windows-x86_64
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
cowen daemon start
```

## 📖 详细文档

- [架构设计 (Architecture)](docs/ARCHITECTURE.md) - 深入了解 `RequestDecorator`、插件系统与安全性设计。
- [动态规约发现 (Dynamic Spec)](docs/TECHNICAL_SPEC.md#26-动态-openapi-发现-dynamic-openapi-discovery) - 详述 Spec 抓取、聚合与 TTL 缓存机制。
- [命令指南 (Commands)](docs/COMMANDS.md) - 全量的子命令参数说明（Api, Auth, Dlq, Log, Profile）。

## 🛠️ 开发者指南

本项目遵循严格的 **TDD (测试驱动开发)** 流程。

### 常用开发命令

```bash
# 运行命名规范验证测试
bash tests/makefile_test.sh
# 运行单元测试
cargo test
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
