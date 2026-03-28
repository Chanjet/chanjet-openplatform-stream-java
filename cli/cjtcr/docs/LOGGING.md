# cjtcr 日志与遥测指南 (Logging & Telemetry)

`cjtcr` 提供了一套工业级的结构化遥测系统，确保在生产环境下能够进行精细化的审计与故障排查。

## 📁 1. 日志域 (Log Domains)

日志被自动路由到四个独立的领域，以分离关注点：

1. **`sys.log`**：系统生命周期日志（启动、配置加载、核心组件状态）。
2. **`audit.log`**：安全审计日志（记录所有通过 CLI 或 Proxy 发起的敏感 API 调用）。
3. **`stream.log`**：Stream Gateway 桥接日志（记录长连接状态与心跳）。
4. **`dlq.log`**：异常消息日志（记录所有进入死信队列的消息详情）。

## ⚙️ 2. 滚动配置 (Log Rotation)

您可以在 `~/.cjtc/default.yaml`（或相应 Profile 的配置文件）中自定义日志的滚动策略：

```yaml
log:
  level: info           # 默认级别: debug, info, warn, error
  max_size_mb: 100      # 单个日志文件最大大小 (单位: MB)
  max_files: 5          # 每个域最多保留的历史切片数量 (自动清理)
```

### 滚动逻辑
- **大小切片**：当日志超过 `max_size_mb` 时，自动将其重命名（如 `sys.log.1`）并创建新文件。
- **自动回收**：当切片数量超过 `max_files` 时，最旧的切片将被彻底删除，防止磁盘空间耗尽。

## 🔍 3. 运维实战

### 实时追踪 (Follow)
使用 `log view` 命令可以像 `tail -f` 一样实时观察系统运行状态：

```bash
# 实时观察审计日志，查看谁在调用什么 API
cjtcr log view audit --follow

# 同时查看最后 50 行
cjtcr log view sys --lines 50 -f
```

### 审计检索
每个审计日志条目都是 JSON 格式，方便使用 `jq` 等工具进行离线分析：

```bash
cat ~/.cjtc/log/audit.log | jq 'select(.fields.method == "POST")'
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
