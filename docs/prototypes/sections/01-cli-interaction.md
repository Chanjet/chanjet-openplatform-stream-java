# CLI 交互流设计 (CLI Interaction Design)

## 1. 存储初始化 (Store Init)
扩展 `init` 命令，增加存储后端选择。

```bash
# 场景 1: 本地文件存储 (Legacy)
cowen init --profile dev

# 场景 2: MySQL 共享存储
cowen init --profile prod \
  --store mysql \
  --db-url "mysql://user:pass@host:3306/db" \
  --encrypt-key "<ENCRYPT_KEY>"

# 场景 3: 混合存储模式 (Redis + PG)
cowen init --profile high-load \
  --store postgres \
  --db-url "postgres://..." \
  --cache redis \
  --cache-url "redis://127.0.0.1:6379"
```

## 2. 状态检查 (Status Check)
```bash
cowen status
# 预期输出:
# Storage: Distributed (MySQL + Redis)
# Connection: [OK]
# Active Profile: prod
```

---
*关联 PRD：[功能清单 - 存储层抽象](../../prd/sections/03-feature-list.md)*
