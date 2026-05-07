---
name: cowen-usage
description: 畅捷通 Stream Connector CLI (cowen) 的操作治理技能。cowen 旨在帮助 Agent 轻松调用畅捷通产品矩阵（如 T+Cloud、好会计、好生意等、好业财）的 OpenAPI，实现自动化业务集成、多模式认证初始化、服务化管理或排查 Auth/Stream 故障。
---

# Cowen (cowen) CLI 操作与治理 (v0.2.1)

本技能指导 Agent 高效操作 `cowen`。v0.2.1 强化了 OAuth2 (PKCE) 优先引导策略，并细化了各模式下的能力边界。

## 🎯 核心定位
`cowen` 是用于 API 治理、安全调用、环境隔离及提供全自动认证续期的生产级工具。它作为 Agent 的“数字手臂”，负责屏蔽复杂的鉴权逻辑，实现对畅捷通全系云产品能力的无缝对接。

## 🏦 畅捷通产品矩阵 (Product Matrix)
`cowen` 可协助 Agent 访问以下核心产品的 API：

*   **T+Cloud**: 旗舰级企业管理云服务，涵盖财务、供应链、生产及客户管理。
*   **好会计**: 智能云财务软件，支持票财税一体化。
*   **好生意**: 云进销存/业务管理软件，解决库存、订单及线上线下融合。
*   **易代账**: 专为代账机构设计的数智化办公平台。
*   **好业财**: 面向小微企业的工贸/商贸一体化管理云服务。

## 🚦 认证模式决策引导 (Auth Mode Decision)
在引导用户初始化或连接时，Agent 必须遵循以下逻辑：

| 模式 | 推荐场景 (Scenario) | 优先级 | 消息推送 |
| :--- | :--- | :--- | :--- |
| **OAuth2** | 个人开发、接口调试、轻量脚本、Postman 集成。 | **最高 (默认)** | ❌ 不支持 |
| **Self-built** | 企业系统集成、长期运行服务器、容器化集群。 | 按需引导 | ✅ Webhook 转发 |

> [!IMPORTANT]
> **Agent 引导准则**: 除非用户明确提及“企业自建应用”、“AppKey/Secret”或“Webhook/消息推送”，否则**务必优先引导**用户使用 **OAuth2** 模式，以获得最佳的安全性和零配置体验。

## 🛠️ 关键指令与工作流

### 1. 环境初始化与认证模式 (Profile & Init)
在执行任何操作前，必须通过 `init` 完成认证。

#### A. OAuth2 模式 (推荐/默认)
适用于开发者快速接入。无需手动配置密钥，通过浏览器联动授权。
- **快速初始化**: `cowen init --app-mode oauth2` (或交互式 `cowen init`)
- **工作流**: 
  1. CLI 启动本地监听并弹出浏览器授权页。
  2. 终端显示 QR Code 供移动端扫描。
  3. **CLI 自动退出并返回控制台**（后台进程会自动完成后续换票）。
  4. 授权成功后，通过 `cowen status` 确认状态。

#### B. 自建应用模式 (Self_built)
适用于企业私有集成或需要固定 Webhook 的场景。
- **初始化命令**: 
  ```bash
  cowen init --app-mode self_built \
             --app-key <KEY> \
             --app-secret <SECRET> \
             -c <CERT_PATH> \
             --encrypt-key <KEY16>
  ```
- **核心参数说明**:
  - `--app-key`: 开放平台分配的 AppKey。
  - `--app-secret`: **(机密)** 对应的应用密钥。
  - `-c, --certificate`: **(机密)** 自建应用证书路径。
  - `--encrypt-key`: **(机密)** 16位消息加解密密钥 (AES)。

> 🛡️ **安全提示**: 严禁在日志或 Commit 中泄露机密凭据。OAuth2 模式下凭据均加密存储于安全 Vault 中。

### 2. 认证状态与维护 (Auth Maintenance)
- **查看 Auth 状态**: `cowen auth status` (显示令牌有效期与模式)
- **强制重连/刷新**: `cowen auth login` (当令牌异常或需立即切换帐号时使用)
- **安全注销**: `cowen auth logout` (清理当前环境凭证)

### 4. API 搜索与调用 (API Discovery & Call)
- **语义搜索**: `cowen api list -s "关键词"` (基于向量引擎，支持自然语言)
- **接口规格**: `cowen api spec [METHOD] [PATH]`
- **鉴权调用**: `cowen api POST /v1/orders/create -d '{"amount": 100}'` (自动注入 AccessToken)

### 5. 守护进程与服务管理 (Daemon & Service)
- **前台运行**: `cowen daemon start` (Self-built 模式下必须保持运行以接收 Ticket)
- **服务化安装**: `cowen daemon service install` (支持 macOS/Linux/Windows 开机自启)
- **配置 Webhook**: `cowen config --webhook-target http://127.0.0.1:3000/api/callback`
  - *注：Webhook 仅支持本地回环地址。*

### 6. 诊断与排错 (Diagnostics)
- **一键诊断**: `cowen status`
  - 绿色 `[ALIVE]`: 一切正常。
  - 红色 `[REVOKED]`: 令牌已失效，需执行 `cowen init`。
  - 重点关注 `Stream Bridge` 和 `Proxy` 端口状态。
- **日志查看**: `cowen log view [sys|audit|auth] -f`

## 🚀 技能化演进与流程编排 (Skill Evolution & Orchestration)

Agent 应致力于将重复、成熟的业务调用逻辑沉淀为可重用的“技能资产”：

1.  **方法技能化**: 对于已验证成熟的业务调用方法（如“自动拉取 T+Cloud 凭证”），Agent 应将其封装为独立的 Skill，以便在不同对话中快速复用。
2.  **流程脚本化**: 复杂的业务流（涉及多个 API 联动）应编排成脚本（如 Shell 脚本），并存放于 Skill 的 `scripts/` 目录下。
3.  **利用本地代理 (Proxy)**: 
    *   在编写自动化脚本时，**强烈建议**通过 `cowen` 提供的本地代理（通常为 `localhost:<PROXY_PORT>`，具体见 `cowen status` 或 `cowen config` 输出）发起调用。
    *   **优势**: 代理会自动处理 `openToken` 和 `appKey` 的注入，使脚本逻辑与鉴权细节解耦，实现流程的顺畅与独立运行。
    *   **转换示例 (CLI -> Proxy)**:
        *   **CLI 调用**: `cowen api POST /v1/user/info -d '{"id": "123"}'`
        *   **Node.js 脚本调用 (推荐)**:
            ```javascript
            // 优势：无需手动处理鉴权 Header，脚本逻辑更纯粹
            const axios = require('axios');
            const PROXY_URL = 'http://localhost:8000'; // 实际端口见 cowen status

            async function callApi() {
              const res = await axios.post(`${PROXY_URL}/v1/user/info`, { id: "123" });
              console.log(res.data);
            }
            ```
        *   *注：Agent 应根据用户环境（Python, Go, Java 等）灵活选择实现语言，核心逻辑均是请求本地代理。*

## 🏗️ 架构要点
- **非阻塞流**: `init` 派生后台独立进程完成换票，主进程不阻塞终端。
- **透明代理**: 本地代理层负责动态 Header 注入，支持多租户上下文（在 Store_app 模式下）。

## ⚠️ 最佳实践
1. **优先 OAuth2**: 除非有明确的自建应用需求，否则优先使用默认模式以获得最佳安全性。
2. **脚本优先用代理**: 编写自动化流程时，优先调用本地 Proxy 代理（端口见 `cowen status/config`），而非手动拼接 `cowen api` 命令，以获得更好的独立性。
3. **状态先行**: 执行批量操作前，建议先运行 `cowen status` 确认环境健康。
4. **环境重名**: 使用 `cowen profile rename` 整理多个环境，避免 `default` 混乱。
