# 动态行为与微观算子 (MLDT)

## 1. HybridStore 读写算子 (Hybrid Write-Through Logic) {#MLDT_HYBRID}

### 场景：执行 `set(profile, key, value)`
1. **[持久化先行]**: 调用 `PersistenceStore.set(profile, key, value)`。
2. **[事务检查]**: 若持久化失败，立即返回 `Err`，不操作缓存，确保 DB 绝对权威性。
3. **[缓存同步]**: 若持久化成功，调用 `CacheStore.set(profile, key, value)`。
4. **[容错处理]**: 若缓存同步失败，记录 `WARN` 日志但不中断流程（允许缓存暂时不一致，后续通过读穿透自愈）。

### 场景：执行 `get(profile, key)`
1. **[缓存命读]**: 调用 `CacheStore.get(profile, key)`。
2. **[命读成功]**: 若返回数据，直接返回。
3. **[缓存失效/穿透]**: 若缓存中无数据或报错：
   - 调用 `PersistenceStore.get(profile, key)`。
   - 若 DB 有数据，**[回填缓存]**：调用 `CacheStore.set` 并返回结果。
   - 若 DB 无数据，返回 `NotFound`。

## 2. SQL 执行与事务处理算子 {#MLDT_SQL}
- **连接获取**: `pool.acquire()` 带有 3s 超时逻辑。
- **SQL 模板**:
  - `SELECT value FROM cowen_storage WHERE profile = ? AND key = ?`
  - `INSERT INTO cowen_storage ... ON DUPLICATE KEY UPDATE value = ?` (MySQL 语法适配)

## 3. 商店应用令牌自愈算子 (Store Auth Recovery Logic) {#MLDT_STORE_AUTH}

### 流程描述：
1. **[感知失效]**: 拦截业务调用时，检测到 `access_token` 已过期。
2. **[首轮尝试]**: 使用 `refresh_token` 执行标准刷新动作。
3. **[自愈判定]**: 若标准刷新返回 4007 (invalid_grant) 或 4029 (session expired)。
4. **[凭据召回]**: 
   - 调用 `Store.get(profile, "user_permanent_code")` 或 `org_permanent_code`。
   - 若本地无永久授权码，返回 `AuthRevoked` 错误并提示重新授权。
5. **[换票重试]**: 
   - 携带永久授权码调用开放平台换票接口。
   - **[原子更新]**: 成功后，同步调用 `HybridStore.set` 更新 `access_token` 与 `refresh_token`。
6. **[业务放行]**: 使用新票据重新发起最初被拦截的业务请求。

---
*关联契约：[HybridStore 编排契约](./02-contracts.md#CONTRACT_HYBRID)*
