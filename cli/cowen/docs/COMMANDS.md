# cowen 命令指南 (Commands v0.3.5)

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
查看或修改配置信息。
- `cowen config`: 查看活跃 Profile 与全局配置的组合结果。
- `cowen config set <KEY> <VALUE>`: 动态修改当前活跃 Profile 的局部配置。
- `cowen config set <KEY> <VALUE> --global`: (v0.3.5+) 动态修改全局基础设施配置 (`app.yaml`)，对所有 Profile 共享并即时生效（如 `log.level`, `security.ssrf_whitelist`）。

### `reset` - 系统重置 (v0.3.5+)
一键重置清理，恢复系统初始状态。基于 `Resettable` 插件化架构实现。
- `--dry-run`: 仅生成并打印出计划清理的物理资源清单（包括 SQLite 文件路径、Redis 清理模式、本地模型缓存、日志轮转目录、文件锁等），但不产生任何物理删除或修改的副作用。
- 执行不带 `--dry-run` 的命令会物理抹除上述所有已注册的状态介质。

---

## 🔍 2. 接口能力与搜索插件 (Api, Search Plugins)

### `api list` - 智能搜索
- `cowen api list`: 列出已授权 API。
- `-s, --search`: 语义搜索。例如 `cowen api list -s "查询余额"`。
- `--refresh`: 强制从平台同步最新的 OpenAPI 规约。

### 搜索插件配置 (v0.3.1+)
Cowen 支持通过动态链接库扩展搜索能力。在 `app.yaml` 中配置：
```yaml
search:
  enabled: ["embedding"] # 显式启用的插件列表。若要彻底禁用 AI 搜索，请保持此数组为空 []。
  plugins:
    embedding: "/usr/local/lib/cowen/libcowen_search_embedding.dylib" # 插件名称与物理路径映射
```

### `api spec` - 规约详情
查看指定接口的文档定义或原始 JSON 片段。

### `plugins` - 本地动态扩展管理 (Legacy / 高阶调试)
> **注意**: 核心生态已重构并收敛至原生的 `cowen-capabilities`，此系列指令目前仅保留用于高级开发者在本地测试加载 `.dylib` / `.so` 格式的底层 C-ABI 扩展。
动态扫描、启用或禁用本地扩展插件。
- `cowen plugins list`: 扫描并列出 `~/.cowen/plugins/` 目录下的可用扩展插件（检测其导出的 C-ABI 接口与描述）。
- `cowen plugins enable <NAME>`: 启用指定的插件并将其注册到全局配置中。
- `cowen plugins disable <NAME>`: 禁用指定的插件。

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

### `auth logout` / `auth reset`
清理本地内存与 Vault 中的 Token 缓存，退出登录会话。

### `auth token`
获取或主动续期刷新当前 Profile 的 AccessToken。
- `--refresh`: 强制立即向开放平台执行 Token 寿命续期刷新。

### `auth reload`
从共享存储中强制同步最新凭据数据到当前工作上下文，并向运行中的 Daemon 进程发送重载信号以即时同步状态。

---

## ⚙️ 4. 系统与后台 (Daemon, System, Store, Doctor)

### `daemon start`
启动长连接桥接器、反向代理与自动续约引擎。
- `--enable-proxy`: 同时开启本地 HTTP 代理网关。
- `--no-proxy`: 强制禁用代理网关，仅做流消息桥接。
- `--foreground`: 在控制台前台阻塞式挂起运行以方便直接观察日志流。
- `-a, --all`: 批量为所有已配置激活的 Profile 环境一并启动后台服务。
- **自适应刷新 (v0.3.1+)**: 后台维护循环会根据 Token 剩余寿命自动计算下一次检查时间（80% 寿命规则 + 随机抖动），显著降低系统开销。

### `daemon stop`
优雅停止运行中的后台守护进程服务。
- `-a, --all`: 批量停止当前机器上运行的所有 Profile 守护进程。

### `daemon restart`
重启守护服务并应用最新加载的配置文件参数。
- `--enable-proxy` / `--no-proxy`: 覆盖启用或关闭代理网关能力。
- `-a, --all`: 批量重启所有已配置的 Profile 守护进程。

### `daemon reload` (v0.3.5+)
在不停止主控服务 Master 守护的前提下，平滑热重启特定的工作 Worker 子进程（例如重新加载新换取的租户 Token）。
- `-a, --all`: 批量热重启所有激活环境的工作子进程。

### `daemon service` (v0.3.5+)
管理操作系统的系统级自启动服务单元。
- `cowen daemon service install`: 将后台守护服务安装到操作系统开机自启动单元中（支持 Linux `systemd` / macOS `launchd` / Windows `Service Control Manager (SCM)`）。
- `cowen daemon service uninstall`: 从系统服务管理器中安全卸载守护进程开机单元。
- `cowen daemon service status`: 诊断操作系统服务管理器中当前单元的运行生命周期状态。

### `status` / `system status`
检查并输出 CLI 的运行状态指标，包含 Daemon 运行状态、Store 连通性、Auth 及 AI 模块的健康度。
- `-a, --all`: 扫描并诊断系统所有存在的 Profile 的详细运行状态矩阵。

### `doctor` - 环境诊断 (v0.3.1+)
运行一键诊断工具，深度检查网络、存储、插件加载、版本一致性及权限问题。
- `-p, --profile <NAME>`: 指定要诊断的环境 Profile。
- `-v, --verbose`: 开启详细诊断模式，包含插件哈希校验与网络延迟测试。
- `--fix`: 尝试自动修复发现的问题（如 SQLite 存储 Schema 自动更新或迁移）。
```bash
cowen doctor --verbose
```

### `store status`
检查当前配置的主存储后端与缓存连接性及健康状态。

### `store set`
配置全局存储后端与缓存的连接参数与引擎类型。
- `--store`: 主存储数据库引擎类型 (可选 `sqlite`, `innerdb`, `mysql`, `postgres`, `redis`, `local`)。
- `--db-url`: 数据库连接 URL 地址。
- `--cache`: 全局缓存引擎类型 (如 `redis`, `memory`, `none`)。
- `--cache-url`: 缓存连接的物理 URL 地址。

### `store migrate` (v0.3.5+)
在不同的底层存储后端之间安全地迁移已保存的配置与凭据状态。
- `--to <URL>`: 迁移的目标数据库连接 URL 地址，如 `sqlite:data/new.db`。
- `--mode <MODE>`: 数据迁移模式。支持 `clone` (复制数据) 或 `move` (物理移动数据，迁移后抹除源数据)。默认值为 `clone`。

---

## 📦 5. 运维审计 (Log, Dlq)

### `log list`
查看当前的日志域列表（sys, audit, stream, dlq）及其对应的物理审计文件信息。

### `log view <DOMAIN>`
查看审计文件或实时跟踪多域日志流的控制台输出。
- `--follow`: 实时追踪日志流水。
- `-n`: 指定默认在尾部展示日志审计文件的尾部行数 (默认 10)。

### `dlq list`
列出当前死信队列 (DLQ) 中因为网络或本地重试耗尽而堆积的异常事件。
- `--page`: 要查看的分页页码 (默认 1)。
- `-n`, `--page-size`: 每页显示的死信记录条数限制 (默认 20)。

### `dlq retry <ID>`
根据唯一事件 ID (UUID) 手动触发指定死信消息重新投递重试。

### `dlq purge`
物理抹除死信队列中堆积的所有历史事件（慎用）。

### `events` - 事件回溯 (v0.3.5+)
查看过去的系统事件流与故障状态切换轨迹，提供细粒度的诊断回溯。
- `-p, --profile <NAME>`: 仅过滤特定 Profile 环境的事件。
- `-n, --limit <LIMIT>`: 展示最近的事件条数 (默认 20)。

---

## ⌨️ 6. 其它

### `completion` - 自动补全
自动生成或安装命令行自动补全脚本 (支持 Zsh, Bash, Fish 和 PowerShell)。
- `--install`: 自动安装补全脚本到当前用户的配置中。
- `--uninstall`: 从当前用户的配置中卸载补全脚本。

### `version` - 版本信息
获取当前 CLI 构建的版本、Build ID 及构建时间。
- `-o, --format <FORMAT>`: 输出格式 (`text`, `json`)，默认为 `text`。

---
© 2026 Chanjet Advanced Agentic Coding Team.
