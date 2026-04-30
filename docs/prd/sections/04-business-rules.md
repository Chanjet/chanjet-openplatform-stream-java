# 业务规则与边界规则 (Business Rules)

## 1. 存储模式全局一致性规则 {#RULE_STORAGE_MUTEX}
- **规则描述**：Cowen 存储方式与应用实例（Binary 部署）全局绑定，而非 Profile 级别。
- **强制约束**：当前应用要么所有 Profile 统一在硬盘上配置（`local`），要么统一在远程存储（`shared`，如 mysql/redis）上配置。
- **冲突处理**：若检测到 Profile 间存在存储引擎混用（如 Profile A 指定 local，Profile B 指定 mysql），系统必须报错退出。

## 2. 混合存储数据归类规则 {#RULE_HYBRID_DATA}
- **短效数据 (Cache Only)**：
  - OAuth2 `access_token` (缓存，默认 TTL=2h)
  - 临时 nonce、session
- **长效数据 (Persistent Only)**：
  - 应用配置 (app_key, webhook_target)
  - OAuth2 `refresh_token`
  - 关键安全证书、加密密钥

## 3. 分布式并发与幂等规则 {#RULE_DISTRIBUTED_LOCK}
- **Token 刷新仲裁**：多个 Sidecar 实例同时检测到 Token 过期时，必须通过 **分布式锁** 竞争刷新权。
- **消息转发**：Webhook 收到消息时，Cowen 仅执行“去壳”操作并转发至业务系统。消息的幂等性与乱序防御逻辑由业务系统自行处理。

## 4. 重绑定与连接策略 {#RULE_REBIND}
- **策略**：若用户尝试重新绑定一个已存在的 Profile，系统将执行 **[全量覆盖]**。
- **历史记录**：所有被覆盖的旧配置必须在日志中留存快照。

## 5. 商店应用长效令牌自动恢复规则 {#RULE_STORE_AUTO_RECOVER}
- **触发条件**：当 `refresh_token` 失效（过期或被吊销）且系统中存在有效的“用户/企业永久授权码”时触发。
- **恢复逻辑**：系统应自动调用开放平台相关接口，利用永久授权码重新获取 `access_token` 与 `refresh_token`，并更新持久化存储。
- **仲裁保护**：恢复动作必须受分布式锁保护（参考 [RULE_DISTRIBUTED_LOCK](#RULE_DISTRIBUTED_LOCK)），确保全局一致性。

---
*溯源参考：[PRD 准则 §5.1, §5.5, §5.7](../index.md#directives) (Verified)*
