# cowen 日志与遥测指南 (Logging v0.3.5)

`cowen` 提供了一套工业级的结构化遥测系统。

## 📁 1. 存储路径
所有日志默认存储在 `~/.cowen/logs/` 目录下。

## ⚙️ 2. 配置说明
配置示例 (Profile YAML 或 DB 配置):
```yaml
log:
  level: info           # debug, info, warn, error
  rotation: daily       # daily, hourly
  max_size_mb: 100
  max_files: 7
```

## 🔍 3. 运维实战

### 查看实时流水
```bash
cowen log view audit --follow
```

### 离线分析
审计日志为标准 JSON 格式，可使用 `jq` 检索：
```bash
tail -n 100 ~/.cowen/logs/audit.log | jq 'select(.status >= 400)'
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
