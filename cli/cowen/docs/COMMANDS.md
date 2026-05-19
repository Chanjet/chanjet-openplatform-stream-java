# cowen 命令指南 (Commands v0.3.1)

本文档详述了 `cowen` CLI 所有可用命令及其功能。

## 📁 1. 基础治理 (Init, Config, Reset, Profile)

### `init` - 初始化
配置 Profile 环境、托管安全凭据。
- `--app-mode`: 指定应用类型 (`self_built`, `store_app` 或 `oauth2`)。
- `--app-key` / `--app-secret`: 凭据。
- `--proxy-port`: 设置本地代理端口 (默认 8080)。
- `--certificate`: 指定自建应用证书（SelfBuilt 模式必填）。

### `profile` - 环境切换
管理多套隔离的环境配置。
- `cowen profile list`: 列出所有 Profile。
- `cowen profile use <NAME>`: 切换当前默认 Profile。
- `cowen profile rename <OLD> <NEW>`: 重命名环境及其关联的数据库记录。

### `COWEN_HOME` - 环境变量隔离
通过设置 `COWEN_HOME` 环境变量，可以在同一台机器上运行多个物理隔离的 Cowen 实例。
```bash
export COWEN_HOME=./.cowen_home
cowen init ...
```

### `config` - 查看与管理配置
查看当前活跃 Profile 的非敏感配置信息。
- `cowen config`: 查看全量配置。
- `cowen config set <KEY> <VALUE>`: 动态修改配置（如 `log.level`, `monitor.port`）。

---

## 🔍 2. 接口能力与搜索插件 (Api, Search Plugins)

### `api list` - 智能搜索
- `cowen api list`: 列出已授权 API。
- `-s, --search`: 语义搜索。例如 `cowen api list -s "查询余额"`。
- `--refresh`: 强制从平台同步最新的 OpenAPI 规约。

### 搜索插件配置 (v0.3.1+)
Cowen 支持通过动态链接库扩展搜索能力。在 `main.yaml` 中配置：
```yaml
search:
  enabled: ["embedding"] # 显式启用的插件列表
  plugins:
    embedding: "libcowen_search_embedding.dylib" # 插件名称与物理路径映射
```

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

## ⚙️ 4. 系统与后台 (Daemon, System, Store, Doctor)

### `daemon start`
启动长连接桥接器、反向代理与自动续约引擎。
- `--enable-proxy`: 同时开启本地 HTTP 代理。
- `--foreground`: 前台运行观察日志。
- **自适应刷新 (v0.3.1+)**: 后台维护循环会根据 Token 剩余寿命自动计算下一次检查时间（80% 寿命规则 + 随机抖动），显著降低系统开销。

### `doctor` - 环境诊断 (v0.3.1+)
运行一键诊断工具，检查网络、存储、插件加载、版本一致性及权限问题。
```bash
cowen doctor --verbose
```

### `system status --all`
扫描并诊断系统所有 Profile 的运行状态矩阵。

### `store status`
检查存储后端（如 SQLite）的连接性与健康度。

### `store set --store <TYPE>`
切换全局存储后端 (e.g. `sqlite`, `redis`, `mysql`, `postgres`)。

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
