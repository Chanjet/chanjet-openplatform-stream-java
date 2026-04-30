# 安全与数据冲突仲裁 (Security & Conflict)

## 1. 鉴权与安全
- **敏感数据加密 (At-Rest Encryption)**：存入共享数据库（MySQL 等）的 `app_secret`、`refresh_token` 等敏感字段必须使用 **AES-256-GCM** 加密。
- **密钥获取**：分布式模式下，加密密钥由环境变量或外部秘密管理（Vault/K8s Secret）提供，不再依赖本地机器指纹。
- **存储连接安全**：建议开启 SSL/TLS 加密（非强制）。

## 2. 数据冲突仲裁规则
- **仲裁原则 (Master/Slave)**：
  - 在混合存储模式下，**数据库 (DB) 为权威数据源 (Source of Truth)**。
  - Redis 仅作为高性能副本。若 Redis 数据与 DB 数据发生冲突，以 DB 为准并强制刷新 Redis。
- **并发刷新冲突**：
  - 基于分布式锁。未获得锁的节点必须进入等待或通过轮询检查最新 Token 是否已更新。

---
*关联业务规则：[分布式并发与幂等规则](../../prd/sections/04-business-rules.md#RULE_DISTRIBUTED_LOCK)*
