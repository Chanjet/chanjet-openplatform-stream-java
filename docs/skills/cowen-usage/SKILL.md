---
name: cowen-usage
description: 畅捷通 Stream Connector CLI (cowen) 的操作治理技能。当需要进行 API 搜索、环境切换、接口调用注入鉴权、开启本地代理或排查 Stream 转发故障时使用。
---

# Cowen (cowen) CLI 操作与治理

本技能指导 Agent 高效操作 `cowen`。

## 🎯 核心定位
`cowen` 是用于 API 治理、安全调用、环境隔离及 Stream 桥接的生产级工具。

## 🛠️ 关键指令与工作流

### 1. 环境初始化与配置 (Profile & Init)
在执行任何 API 操作前，必须确保环境已正确初始化。`init` 命令会引导配置并安全存储敏感凭据。
- **初始化命令**: `cowen init --app-key <KEY> --app-secret <SECRET> -c <CERT> --encrypt-key <EKEY>`
  - `--app-key`: 开放平台分配的应用唯一标识。
  - `--app-secret`: **(机密)** 对应的应用密钥。
  - `-c, --certificate`: **(机密)** 自建应用证书。
  - `--encrypt-key`: **(机密/关键)** 消息加解密秘钥，用于 Stream 桥接中对消息体进行对称加密/解密。
- **可选参数**:
  - `--webhook-target`: 配置本地 Webhook 接收地址。
  - `--openapi-url` / `--stream-url`: 覆盖默认的生产/测试环境地址。
- **环境切换**: `cowen profile use <NAME>` (常用: `default`, `inte`, `prod`)
- **查看配置**: `cowen profile current`

> 🛡️ **安全提示**: 严禁在任何公共日志、Commit Message 或 Shell 历史记录中泄露 `AppSecret`、`Certificate` 或 `EncryptKey` 内容。建议通过环境变量或交互式输入传递。

### 2. 语义化 API 搜索 (API Discovery)
- **语义搜索**: `cowen api list -s "意图关键词"`
- **查看详情**: `cowen api spec [METHOD] [PATH]`

### 3. 声明式接口调用 (API Call)
系统自动处理鉴权注入。
- **调用**: `cowen api POST /v1/orders/create -d '{"amount": 100}'`

### 4. 守护进程与代理 (Daemon)
- **启动代理**: `cowen daemon start --proxy-port 8080`
- **查看日志**: `cowen log view [sys|audit|stream|dlq] -f`

## 🏗️ 架构要点
- **鉴权逻辑**: 核心在 `src/auth/decorator.rs`。
- **语义引擎**: 基于 BGE-small-zh (ONNX)。
- **TDD 约束**: 新增功能必须在 `tests/` 中提供对应测试。

## ⚠️ 最佳实践
1. **探索式学习**: `cowen --help` 提供全局概览，每个子命令（如 `api`, `daemon`, `log`）也均支持 `--help`。遇到未记录的参数或功能时，优先通过此方式探索。
2. **优先语义搜索**: 使用 `cowen api list -s` 寻找接口，而非遍历文档。
3. **凭据安全**: 严禁在日志、环境变量记录或 Commit 中泄露 `AppSecret`、`Certificate` 或 `EncryptKey`。
4. **环境确认**: 切换 Profile 后，在执行写操作前务必通过 `cowen profile current` 核实。
