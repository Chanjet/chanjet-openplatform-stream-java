# 进阶运维与自愈指南 (Operations & Resilience)

本文档整理了 `cowen` CLI 的进阶操作技巧，旨在帮助运维人员在复杂网络环境或分布式部署中保持系统稳定。

---

## 🛠️ 故障诊断 (Diagnostics)

当您发现无法接收推送或 API 调用返回 401 时，请优先使用内置诊断工具：

```bash
# 执行全链路自检
cowen diagnose
```

**诊断项包括**：
- **Connectivity**: 检查与畅捷通开放平台（API & WebSocket）的连通性。
- **Vault Status**: 验证敏感存储是否正常解锁（Keychain/File）。
- **Token Health**: 检查当前令牌是否过期，以及本地缓存与远端的一致性。
- **Daemon Pulse**: 检查后台守护进程是否存活及端口占用情况。

---

## 📬 死信队列管理 (Dead Letter Queue)

当本地 Webhook 转发失败（如您的业务系统宕机）时，消息会进入 `DLQ`。

### 1. 查看待处理消息
```bash
# 查看死信摘要
cowen dlq list

# 查看特定消息的详细失败原因（含堆栈）
cowen dlq show <MSG_ID>
```

### 2. 手动触发重试
在您的业务系统修复后，可以批量触发重试：
```bash
# 重试所有死信
cowen dlq retry --all

# 仅重试特定时间段内的消息
cowen dlq retry --since "2024-05-01 12:00:00"
```

---

## 🔄 权限同步与动态发现 (API Discovery)

当您在畅捷通开放平台后台修改了应用的 API 权限（如新增了某个接口的权限）时，本地缓存的規约可能不会立即更新。

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
- 即使 10 个实例同时发现令牌即将过期，也只有一个实例会发起网络刷新请求。
- 其他实例会进入短暂等待，并随后从共享存储中直接读取新令牌。

### 2. 状态一致性检查
如果您怀疑集群间存在数据偏移，可以运行：
```bash
# 强制同步集群状态并校验存储校验和
cowen store status --verify
```

---

## ⌨️ 效率工具 (Efficiency)

### 1. 命令补全 (Shell Completion)
支持 Zsh, Bash, Fish 和 PowerShell 的自动补全：
```bash
# 以 Zsh 为例
cowen completions zsh > /usr/local/share/zsh/site-functions/_cowen
source ~/.zshrc
```

### 2. 状态快速预览
在终端左侧或监控面板中，可以使用以下命令获取简洁的状态概要：
```bash
# 获取精简版状态 (适合脚本监控)
cowen status --short
```

---
© 2026 Chanjet Advanced Agentic Coding Team.
