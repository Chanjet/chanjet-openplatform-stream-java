---
name: cowen-usage
description: 畅捷通 Stream Connector CLI (cowen) 的操作治理技能。当需要进行 API 搜索、多模式认证初始化、服务化管理或排查 Auth/Stream 故障时使用。
---

# Cowen (cowen) CLI 操作与治理 (v0.2.0)

本技能指导 Agent 高效操作 `cowen`。v0.2.0 引入了全自动的 OAuth2 (PKCE) 认证流，简化了接入成本。

## 🎯 核心定位
`cowen` 是用于 API 治理、安全调用、环境隔离及提供全自动认证续期的生产级工具。

## 🛠️ 关键指令与工作流

### 1. 环境初始化与认证模式 (Profile & Init)
在执行任何操作前，必须通过 `init` 完成认证。v0.2.0 支持两种模式：

#### A. OAuth2 模式 (推荐/默认)
适用于开发者快速接入。无需手动配置密钥，通过浏览器联动授权。
- **快速初始化**: `cowen init` (交互式) 或 `cowen init --app-mode oauth2`
- **工作流**: 
  1. CLI 启动本地监听并弹出浏览器授权页。
  2. 终端显示 QR Code 供移动端扫描。
  3. **CLI 自动退出并返回控制台**（后台 Finalizer 进程会自动完成后续换票）。
  4. 授权成功后，通过 `cowen status` 确认状态。

#### B. 自建应用模式 (Self-built)
适用于企业私有集成或需要固定 Webhook 的场景。
- **初始化命令**: `cowen init --app-mode self-built --app-key <KEY> --app-secret <SECRET> -c <CERT>`
- **参数说明**:
  - `--app-key`: 开放平台分配的 AppKey。
  - `--app-secret`: **(机密)** 对应的应用密钥。
  - `-c, --certificate`: **(机密)** 自建应用证书路径。

> 🛡️ **安全提示**: 严禁在日志或 Commit 中泄露 `AppSecret`、`Certificate`。OAuth2 模式下凭据均加密存储于安全 Vault 中。

### 2. 认证状态与维护 (Auth Maintenance)
- **查看 Auth 状态**: `cowen auth status` (显示令牌有效期与模式)
- **强制重连/刷新**: `cowen auth login` (当令牌异常或需立即切换帐号时使用)
- **安全注销**: `cowen auth logout` (清理当前环境凭证，不破坏基础配置)
- **全量重置**: `cowen auth reset` (等同于 logout，语义更一致)

### 3. API 搜索与调用 (API Discovery & Call)
- **语义搜索**: `cowen api list -s "关键词"` (基于向量引擎，支持自然语言)
- **接口规格**: `cowen api spec [METHOD] [PATH]`
- **鉴权调用**: `cowen api POST /v1/orders/create -d '{"amount": 100}'` (自动注入 AccessToken)

### 4. 守护进程与服务管理 (Daemon & Service)
- **前台运行**: `cowen daemon start`
- **服务化安装**: `cowen daemon service install` (支持 macOS/Linux/Windows 开机自启)
- **服务控制**: 使用系统原生指令 (如 `brew services` 或 `sc start cowen`)。

### 5. 诊断与排错 (Diagnostics)
- **一键诊断**: `cowen status`
  - 绿色 `[ALIVE]`: 一切正常。
  - 红色 `[REVOKED]`: 令牌已失效，需执行 `cowen init`。
  - 黄色 `[EXPIRED]`: 正在自动刷新。
- **日志查看**: `cowen log view [sys|audit|auth] -f`

## 🏗️ 架构要点
- **非阻塞流**: `init` 派生后台独立进程完成换票，主进程不阻塞终端。
- **并发刷新锁**: 多进程环境自动竞争文件锁，防止 Refresh Token 冲突失效。

## ⚠️ 最佳实践
1. **优先 OAuth2**: 除非有明确的自建应用需求，否则优先使用默认模式以获得最佳安全性。
2. **状态先行**: 执行批量操作前，建议先运行 `cowen status` 确认环境健康。
3. **环境重名**: 使用 `cowen profile rename` 整理多个环境，避免 `default` 混乱。
