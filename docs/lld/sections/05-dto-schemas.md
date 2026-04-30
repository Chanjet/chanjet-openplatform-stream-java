# 物理模型萃取 (DTO & DDL)

## 1. 数据库 DDL (SQL Schema) {#DDL_STORAGE}

```sql
-- Cowen 共享存储表设计
CREATE TABLE IF NOT EXISTS cowen_storage (
    profile VARCHAR(64) NOT NULL,
    item_key VARCHAR(128) NOT NULL,
    item_value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (profile, item_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- 分布式锁表（针对 SQL 驱动）
CREATE TABLE IF NOT EXISTS cowen_locks (
    lock_name VARCHAR(64) PRIMARY KEY,
    owner_id VARCHAR(64),
    expires_at TIMESTAMP
);
```

## 2. 存储配置 DTO (Storage Config) {#DTO_STORAGE_CONFIG}

| 字段名 | 类型 | 描述 | 备注 |
| :--- | :--- | :--- | :--- |
| `type` | String | 存储类型 (`local`, `mysql`, `postgres`, `redis`) | 必填 |
| `url` | String | 连接字符串 | `shared` 模式必填 |
| `encrypt_key` | String | 数据加密密钥 | 可选，默认由 ENV 提供 |

## 3. 标准存储键字典 (Standard Storage Key Dictionary) {#DTO_KEY_DICT}

| 键名 (Key) | 描述 | 归属模式 | 敏感度 |
| :--- | :--- | :--- | :--- |
| `access_token` | 短效业务访问令牌 | 全模式 | 高 |
| `refresh_token` | 令牌刷新凭据 | 商店应用 | 高 |
| `app_ticket` | 平台推送票据 | 自建应用 | 中 |
| `user_permanent_code`| 用户永久授权码 | 商店应用 | 极高 |
| `org_permanent_code` | 企业永久授权码 | 商店应用 | 极高 |

---
*关联 PRD：[数据映射与实体字典](../../prd/sections/04-business-rules.md)*
