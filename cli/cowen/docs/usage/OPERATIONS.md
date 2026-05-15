# 进阶运维与自愈指南 (Operations & Resilience)

本文档整理了 `cowen` CLI 的进阶操作技巧，旨在帮助运维人员在复杂网络环境或分布式部署中保持系统稳定。

---

## 🛠️ 故障诊断 (Diagnostics)

当您发现无法接收推送或 API 调用返回 401 时，请通过以下内置命令检查系统状态：

```bash
# 查看详细的身份认证与长连接状态
cowen auth status

# 检查全局存储后端与缓存的连通性
cowen store status
```

**关键检查点**：
- **Auth Status**: 确认 `App Access Token` 是否有效。
- **Stream Bridge**: 确认 WebSocket 状态是否为 `ACTIVE`。
- **Store Status**: 确认数据库或 Redis 是否可连通。

---

## 📬 死信队列管理 (Dead Letter Queue)

当本地 Webhook 转发失败（如您的业务系统宕机）时，消息会进入 `DLQ`。

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

## ⌨️ 效率工具 (Efficiency)

### 1. 命令补全 (Shell Completion)
支持 Zsh, Bash, Fish 和 PowerShell 的自动补全：
```bash
# 以 Zsh 为例，安装补全脚本
cowen completion --install
```

---
© 2026 Chanjet Advanced Agentic Coding Team.

---
© 2026 Chanjet Advanced Agentic Coding Team.
