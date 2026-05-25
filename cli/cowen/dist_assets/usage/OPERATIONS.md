# 进阶运维与自愈指南 (Operations & Resilience)

本文档整理了 `cowen` CLI 的进阶操作技巧，旨在帮助运维人员在复杂网络环境或分布式部署中保持系统稳定。

---

## 🛠️ 深度诊断与自检 (Diagnostics & Doctor)

当您发现无法接收推送或 API 调用异常时，除了基础状态检查外，建议使用 v0.3.1 引入的深度体检工具：

### 1. 一键深度体检 (System Doctor)
```bash
# 运行全面诊断（推荐在配置变更或迁移后运行）
cowen doctor

# 运行详细模式（包含插件哈希校验与网络延迟测试）
cowen doctor --verbose
```
**诊断内容包括**：
- **网络探测**：检查 OpenAPI 和 Stream Gateway 的端到端延迟。
- **存储权限**：验证数据库（SQLite/MySQL/Redis）的读写权限与表结构一致性。
- **插件校验**：检查 AI 搜索插件 (`cdylib`) 是否加载成功及其版本指纹。
- **环境隔离**：验证 `COWEN_HOME` 路径下的物理权限。

### 2. 基础状态监控
```bash
# 查看详细的身份认证与长连接状态
cowen auth status

# 检查全局存储后端与缓存的连通性
cowen store status
```

---

## ⚡ 动态配置与热重载 (Dynamic Config & Hot-Reload)

在 v0.3.1+ 中，`cowen` 支持**零停机时间**的动态配置调整，确保生产环境下连接不断流。

### 1. 动态调整日志级别
无需重启 Daemon 进程，即可即时改变日志输出深度（用于在线排查）：
```bash
# 将日志级别动态提升至 debug (v0.3.5+ 全局配置)
cowen config set log.level debug --global
```
*注：系统会通过 `SIGHUP` 信号或内部监听器通知 Daemon 进程，变更会在 1 秒内生效。*

### 2. 指标监控端口
您可以动态修改监控端口，以适配不同的集群安全组：
```bash
# 动态修改全局监控端口
cowen config set monitor.port 9091 --global
```

---

## 📊 指标监控与健康度 API (Metrics & Health)

`cowen` 提供符合 Prometheus 标准的监控端点，支持对接 Grafana 等主流观测工具。

- **健康状态**: `GET http://127.0.0.1:8081/health` -> 返回 `UP` 状态。
- **性能指标**: `GET http://127.0.0.1:8081/metrics` -> 返回 Prometheus 格式的打点数据。

**核心指标清单**：
- `cowen_api_calls_total`: 代理调用总次数。
- `cowen_stream_reconnects_total`: 长连接重连次数（用于评估网络稳定性）。
- `cowen_dlq_size`: 当前死信队列积压数。
- `cowen_token_ttl_seconds`: 令牌剩余有效期（用于告警）。

---

## 🧩 可插拔搜索插件 (Search Plugins)

v0.3.1 引入了基于动态链接库的搜索增强架构，允许在不增加主程序体积的情况下扩展语义搜索能力。该功能完整支持 macOS, Linux 及 **Windows** 操作系统。

- **内置搜索**：默认提供基础字符串匹配。
- **AI 增强**：通过插件支持 ONNX 向量检索。
- **配置与管理**：
    - **自动发现**: 系统会自动扫描插件目录下的候选文件：
        - **macOS**: `.dylib` 或 `.so`
        - **Linux**: `.so`
        - **Windows**: `.dll`
    - **扫描路径**: 优先扫描 `/usr/local/lib/cowen/` (Unix) 或 `cowen.exe` 所在目录。
    - **精细化启闭管理**: 
        - **推荐做法 (持久化)**: 在配置文件 `app.yaml` 的 `search.enabled` 数组中移除插件名称（如 `cowen-search-embedding`），即可彻底禁用对应的 AI 搜索能力。保持数组为空 `[]` 时，系统将仅使用基础字符串匹配。
    - **优雅降级**: 如果插件加载失败，系统会自动降级到 `StringMatch` 模式，确保 API 搜索功能依然可用。

---

## 📬 死信队列管理 (Dead Letter Queue)

当本地 Webhook 转发失败（如您的业务系统宕机）时，消息会进入 `DLQ`。
... (rest of content) ...


### 1. 查看待处理消息
```bash
# 查看死信摘要列表
cowen dlq list
```

### 2. 手动触发重试
在您的业务系统修复后，可以触发重试：
```bash
# 重试指定 ID 的消息 (ID 可通过 dlq list 获取)
cowen dlq retry <MSG_ID>

# 清空死信队列 (谨慎操作)
cowen dlq purge
```

---

## 🔄 权限同步与动态发现 (API Discovery)

当您在畅捷通开放平台后台修改了应用的 API 权限（如新增了某个接口的权限）时，本地缓存的规约可能需要强制刷新。

```bash
# 强制从平台刷新最新的 OpenAPI 规约及授权白名单
cowen api list --refresh
```
*注：此操作会触发重新构建本地向量搜索索引，确保 `api list -s` 能搜到新接口。*

---

## 🌐 分布式与集群一致性 (Cluster Management)

在多节点部署场景下（如 K8s ReplicaSet），多个 `cowen` 实例共用同一个 `Redis` 或 `MySQL`。

### 1. 冲突保护
`cowen` 内部实现了基于分布式锁的 **刷新仲裁机制**：
- 即使多个实例同时发现令牌即将过期，也只有一个实例会发起网络刷新请求。
- 其他实例会进入短暂等待，并随后从共享存储中直接读取新令牌。

### 2. 状态批量查看
如果您需要同时监控多个租户环境：
```bash
# 扫描并输出所有已存在的 Profile 状态
cowen status --all
```

---

## 🧹 系统一键重置与状态清理 (System Reset) (v0.3.5+)

在运维升级、迁移环境或发生重大本地存储故障时，您可能需要彻底清理本地缓存和状态，以便“恢复出厂设置”重新初始化。

`cowen` 支持插件化的二相重置清理机制，确保各组件状态的原子擦除：

### 1. 预览重置计划 (Dry Run)
在执行破坏性清除之前，强烈建议先运行 `--dry-run` 选项以获得确定性预览：
```bash
# 仅生成并输出将要删除的物理文件（数据库、日志、模型、锁）和资源清单，零副作用
cowen reset --dry-run
```

### 2. 正式执行重置
当确认无误后，即可执行无参数重置以物理抹除所有状态介质：
```bash
# 物理删除所有已注册状态，恢复出厂设置
cowen reset
```
*注：系统重置后，本地所有 Profile 将全部丢失，您需要重新运行 `cowen init` 以恢复工作能力。*

---

## ⌨️ 效率工具 (Efficiency)

### 1. 命令补全 (Shell Completion)
支持 Zsh, Bash, Fish 和 PowerShell 的自动补全：
```bash
# 以 Zsh 为例，安装补全脚本
cowen completion --install
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
