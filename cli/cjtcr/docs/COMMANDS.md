# cjtcr 命令指南 (Commands)

本文档详细介绍了 `cjtcr` 所有子命令的用法与参数。

## 📁 1. 配置与环境 (Profile & Init)

### `init` - 初始化
引导式初始化当前 Profile 的配置与加密凭据。

- **必选参数 (管家模式/自建应用)**:
  - `--app-key`: 开放平台 AppKey。
  - `--app-secret`: 开放平台 AppSecret。
  - `-c, --certificate`: 自建应用证书 (Certificate)。
- **可选参数**:
  - `--webhook-target`: 本地 Webhook 接收地址。
  - `--openapi-url` / `--stream-url`: 覆盖默认的平台地址。

```bash
cjtc init --app-key <KEY> --app-secret <SECRET> -c <CERT>
```

### `profile` - 环境管理
`cjtcr` 支持多环境隔离。
- `cjtc profile use <NAME>`: 切换到指定环境（如 `prod`）。
- `cjtc profile current`: 显示当前激活的环境。

---

## 🔍 2. 接口治理 (Api)

### `api list` - 智能检索
- `cjtc api list`: 列出所有 API（带摘要与描述）。
- `cjtc api list -s "关键词"`: 启用 **Neural Search** 语义搜索，基于意图发现接口。
- `-n <TOP>`: 指定搜索结果的数量。

### `api spec` - 规范查看
查看特定接口的详尽定义或原始 OpenAPI 片段。
```bash
cjtc api spec GET /v1/user --raw
```

### `api [METHOD] [PATH]` - 直接调用
声明式调用接口，系统会自动处理鉴权。
```bash
cjtc api POST /v1/orders/create -d '{"id": 1}'
```

---

## 🛡️ 3. 守护进程与代理 (Daemon)

### `daemon start` - 开启服务
在后台启动代理服务器与转发器。
- `--proxy-port <PORT>`: 自定义本地代理端口（默认 8080）。
- `--foreground`: 在前台运行以便观察实时日志。

### `daemon stop` - 停止服务
安全停止所有后台进程。

---

## 📦 4. 故障处理 (Dlq)

### `dlq list` - 查看堆积
列出所有转发失败的消息及其错误原因。

### `dlq retry <ID>` - 手动重试
针对特定消息进行重发，成功后自动移除。

### `dlq purge` - 清空队列
一键清理所有过期的死信记录。

---

## 📈 5. 日志与遥测 (Log)

### `log list` - 列出日志
查看 `sys`, `audit`, `stream`, `dlq` 等日志域的文件状态。

### `log view <DOMAIN>` - 追踪日志
- `cjtc log view sys`: 查看最后 10 行系统日志。
- `--follow` / `-f`: 开启实时追踪模式（类似 `tail -f`）。
- `--lines <N>`: 指定显示的行数。

---

## ⌨️ 6. 其它 (Completion)

### `completion --install` - 自动补全
自动为当前用户的 Shell（Zsh/Bash/Fish）安装命令自动补全脚本。

---
© 2026 Chanjet Advanced Agentic Coding Team.
