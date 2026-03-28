# cjtcr 架构设计 (Architecture)

本文档旨在为未来的开发者详细介绍 `cjtcr` 的核心设计理念、模块划分以及核心能力实现。

## 🏛️ 总体架构

`cjtcr` 采用模块化设计，核心逻辑通过 Trait 进行抽象，确保了良好的可扩展性和可测试性。

- **`core/`**: 核心库（配置、安全、遥测、语义引擎）。
- **`auth/`**: 认证抽象层（Vault、Token 存储、请求装饰器）。
- **`daemon/`**: 守护进程实现（Proxy 代理、Forwarder 转发、DLQ 存储）。
- **`cmd/`**: CLI 命令处理器。

## 🔑 核心组件详情

### 1. RequestDecorator (规约驱动的自动鉴权)

`src/auth/decorator.rs` 是整个系统的“安全网关”。它实现了“规约优先”的设计原则：

- **自动化注入**：不依手动硬编码 Header。它解析 OpenAPI 规约，识别 `appKey`、`appSecret` 或 `openToken` 的位置（通常在 Header 中）。
- **动静结合**：它结合了本地 Vault 中的凭据（静态）与从开放平台动态刷新出的 AccessToken（动态）。
- **多端共用**：无论是 CLI 直接调用还是 Proxy 转发，都强制经过装饰器，保证了行为的一致性。

### 2. Semantic Search Indexer (语义搜索引擎)

`src/core/search.rs` 引入了轻量级 AI 能力：

- **核心模型**：BGE-small-zh-v1.5 (ONNX)。
- **嵌入生成**：使用 `ort` Crate 直接在本地 CPU 上运行向量化推理。
- **倒排索引 + 向量检索**：结合了传统的字符串匹配与深度语义搜索，显著提升了 API 发现的准确率。

### 3. Telemetry System (结构化遙测)

`src/core/telemetry.rs` 实现了工业级的日志管理：

- **多域路由 (Multi-Domain)**：日志被分类为 `sys` (系统)、`audit` (审计)、`stream` (流事件)、`dlq` (异常记录)。
- **滚动策略 (LogRoller)**：支持基于“文件大小”与“时间周期”的双重切片方案。
- **自动清理**：通过 `max_files` 限制自动回收旧日志，防止磁盘爆满。

## 🛡️ 安全性设计

- **本地 Vault 存储**：敏感凭据（AppSecret）经过 AES-GCM 加密，且密钥绑定了机器指纹（Machine ID + Hostname）。
- **审计记录**：所有敏感 API 调用都会自动在 `audit.log` 中留痕，包括调用者身份、目标路径和执行结果。

## 🧪 开发规范 (TDD)

本项目严格执行测试驱动开发 (TDD)。

- **单元测试**：位于各模块的 `tests` 模块或同级 `_test.rs`。
- **集成测试**：通过 `make build-test` 生成的 `cjtc-test` 二进制文件，可在独立的 `.cjtc-test` 目录下进行全流程验证。

---
© 2026 Chanjet Technical Architecture Committee.
