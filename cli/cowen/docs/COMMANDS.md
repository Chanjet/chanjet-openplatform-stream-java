# cowen 命令指南 (Commands v0.3.0)

本文档详述了 `cowen` CLI 所有可用命令及其功能。

## 📁 1. 基础治理 (Init, Config, Reset)

### `init` - 初始化
配置 Profile 环境、托管安全凭据。
- `--app-mode`: 指定应用类型 (`self_built`, `store_app` 或 `oauth2`)。
- `--app-key` / `--app-secret`: 凭据。
- `--proxy-port`: 设置本地代理端口 (默认 8080)。

### `COWEN_HOME` - 环境变量隔离
通过设置 `COWEN_HOME` 环境变量，可以在同一台机器上运行多个物理隔离的 Cowen 实例。
```bash
export COWEN_HOME=./.cowen_home
cowen init ...
```

### `config` - 查看配置
查看当前活跃 Profile 的非敏感配置信息。支持 `-o json` 输出。

### `reset` - 重置环境
物理粉碎当前 Profile 的所有本地配置、缓存与 Vault 凭据。

---

## 🔍 2. 接口能力 (Api)

### `api list` - 智能搜索
- `cowen api list`: 列出已授权 API。
- `-s, --search`: 语义搜索。例如 `cowen api list -s "查询余额"`。
- `--refresh`: 强制从平台同步最新的 OpenAPI 规约。

### `api spec` - 规约详情
查看指定接口的文档定义或原始 JSON 片段。

### `api [METHOD] [PATH]` - 接口调用
直接发起受控请求，系统自动处理签名与 Token。
```bash
cowen api GET /v1/user
cowen api POST /v1/orders -d '{"amount": 100}'
```

---

## 🛡️ 3. 身份与安全 (Auth, Vault)

### `auth status`
检查当前环境的 Token 健康度与 AppTicket 状态。

### `auth login`
手动触发网络换票流程。
- `--force`: 强制使本地缓存失效并重新换票。

### `auth logout`
清理本地内存与 Vault 中的 Token 缓存。

---

## ⚙️ 4. 系统与后台 (Daemon, System, Store)

### `daemon start`
启动长连接桥接器与反向代理。
- `--enable-proxy`: 同时开启本地 HTTP 代理。
- `--foreground`: 前台运行观察日志。

### `system status --all`
扫描并诊断系统所有 Profile 的运行状态矩阵。

### `store status`
检查存储后端（如 SQLite）的连接性与健康度。

### `store set --store <TYPE>`
切换全局存储后端 (e.g. `local`, `innerdb`)。

---

## 📦 5. 运维审计 (Log, Dlq)

### `log list`
查看当前的日志域列表（sys, audit, stream, dlq）。

### `log view <DOMAIN>`
- `--follow`: 实时追踪日志流水。
- `-n`: 指定起始行数。

### `dlq list`
查看堆积的失败事件。

### `dlq retry <ID>`
手动触发死信重发。

---

## ⌨️ 6. 其它

### `completion --install`
自动安装 Shell 命令补全脚本。

---
© 2026 Chanjet Advanced Agentic Coding Team.
