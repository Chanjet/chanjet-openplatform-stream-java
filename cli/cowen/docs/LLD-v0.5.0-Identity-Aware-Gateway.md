# LLD v0.5.0 - Identity-Aware Gateway (执行级蓝图设计)

## 1. 物理模型契约 (Physical Model Contracts)

### 1.1 YAML 配置声明模型 (Configuration Schema)
```yaml
# 向下完全兼容的平铺结构，由 --config 或 --profile 指定的特定环境加载
proxy_port: 8081  # 透明正向出口代理端口 (Egress)
webhook_target: "http://127.0.0.1:5000/callback" # 长链接异步业务消息接收地址

gateway:
  bind_address: "0.0.0.0:8080"
  upstream_url: "http://127.0.0.1:3000"
  auth_sync_hook: "http://127.0.0.1:3000/internal/auth-hook" # 第一轨换票阻塞同步回调
  auth_routing:
    mode: "PERMISSIVE"  # STRICT | PERMISSIVE
    require_rules: ["/api/**", "/invoice/**"]
    bypass_rules: []
    
apply_plugins:
  - "token-exporter"
```

### 1.2 JWE (JSON Web Encryption) 载荷状态机
```json
// Header
{
  "alg": "dir",
  "enc": "A256GCM",
  "kid": "key_uuid_123"
}
// Payload (解密后内容)
{
  "org_id": "org_123",
  "user_id": "user_456",
  "open_token": "bearer_xxx_yyy",
  "idle_exp": 1718101800, // 滑动过期 (当前时间 + 30m)
  "abs_exp": 1718186400,  // 绝对过期 (通常为 24h)
  "fp": "sha256(Client_IP + User_Agent)" // 指纹加固
}
```

### 1.3 JWKS Store 持久化模型
**键值**: `cowen:system:jwks` (在 Redis 或 MySQL 中)
**格式**:
```json
{
  "keys": [
    {
      "kid": "key_uuid_123",
      "kty": "oct",
      "k": "base64_url_encoded_256bit_key",
      "created_at": 1717200000,
      "status": "ACTIVE"  // 唯一签发态
    },
    {
      "kid": "key_uuid_000",
      "status": "ROTATED" // 历史解密态
    }
  ]
}
```

## 2. 确定性逻辑算子 (Deterministic Logic Operators)

### 2.1 Ingress 入流量全局拦截与清洗算子 (Ingress Operator)
**输入**: HTTP Request `Req`
**输出**: HTTP Response

1. **[CORS 预检穿透]**:
   `IF Req.method == "OPTIONS" THEN RETURN HTTP 200/204 放行给后端;`
2. **[全局 Code 拦截优先权]**:
   `IF Req.query 包含 'code' THEN`:
   a. 请求开放平台换取 `open_token`.
   b. `IF auth_sync_hook 被配置 THEN`:
      - 阻塞调用 `POST auth_sync_hook` (附带 org_id, user_id).
      - 从 Webhook 响应提取 `Set-Cookie` (保存为 `isv_cookie`).
   c. 生成新的 `JWE Payload`，通过 Store 中 ACTIVE 的 `kid` 密钥加密。
   d. 生成去除 `code` 参数后的 URL (`Clean_URL`).
   e. 构造 `HTTP 302 Redirect` 至 `Clean_URL`。
   f. 在响应头附加 `Set-Cookie: cowen_sess_id=<JWE>; HttpOnly; Secure; SameSite=Lax`.
   g. `IF sync_hook 存在 isv_cookie THEN` 附加下发业务 Cookie.
   h. `RETURN`.
3. **[黑白名单路由决断]**:
   a. `IF (mode == STRICT AND Req.path 在 bypass_rules 中) OR (mode == PERMISSIVE AND Req.path 不在 require_rules 中) THEN is_auth_required = false;`
   b. `ELSE is_auth_required = true;`
4. **[会话自省与校验]**:
   a. `EXTRACT JWE` 从 `Req.cookies.cowen_sess_id`.
   b. `IF 提取失败 OR 解密失败 OR fp() != JWE.fp THEN`:
      - `IF is_auth_required == false THEN` 继续第5步 (无状态放行).
      - `ELSE IF Req.headers.Accept == "application/json" THEN RETURN HTTP 401 (附带 login_url)`.
      - `ELSE RETURN HTTP 302 (附带 state = Req.path) 至免登地址`.
   c. `IF now() > JWE.abs_exp OR now() > JWE.idle_exp THEN` 判定为超时，执行上一步的 401/302 拦截.
5. **[双时间戳离散滑动续期] (仅当 JWE 合法时)**:
   a. `IF JWE.idle_exp - now() <= 10_minutes THEN`:
      - 触发刷新：基于旧载荷，重写 `idle_exp = now() + 30m`，生成 `New_JWE`。
      - 挂载 Hook：当 Proxy 将后端响应返回给前端时，在 Response Header 追加 `Set-Cookie: cowen_sess_id=<New_JWE>`.
6. **[上下游透传]**:
   a. `IF JWE 合法 THEN` 追加请求头 `x-org-id: JWE.org_id`, `x-user-id: JWE.user_id`.
   b. `IF token-exporter 插件被启用 THEN` 追加 `X-Cowen-Open-Token` 和 Host Vault 参数。
   c. 将请求 Proxy 转发给 `upstream_url`。

### 2.2 Egress Native Proxy 算子
**角色**: 作为 `127.0.0.1:8081` 监听的正向代理
1. 接收 ISV 发来的对外请求。
2. 提取 `Req.headers.cowen_sess_id` 或根据 ISV 提供的主键找到网关级会话缓存。
3. 解密 JWE 获取真实的 `open_token`。
4. 拼装标头 `Authorization: Bearer <open_token>` 并改写目标地址至开放平台实际网关。
5. 执行网络调用并原样返回结果给 ISV。

### 2.3 自治密钥轮转算子 (JWKS Rotation Operator)
**触发条件**: 进程启动时 / 周期性后台协程 (每小时)
1. `GET cowen:system:jwks FROM Store`
2. 查找状态为 `ACTIVE` 的 key。
3. `IF active_key 不存在 OR now() - active_key.created_at >= 30_days THEN`:
   a. 尝试获取分布式锁 `lock:jwks_rotate`.
   b. 获取锁成功后，再次 Check 避免并发修改。
   c. 将原有 ACTIVE 变为 ROTATED。
   d. 生成随机 256bit 串 `New_Key` 和新 UUID `kid`，状态置为 ACTIVE，Push 进列表。
   e. 原子的 `SET` 回 Store，并通知本地内存更新。
   f. 释放锁。

## 3. 健壮性重试矩阵 (Robustness Retry Matrix)

| 动作类型 (Action) | 异常分类 (Error Scenario) | 重试模型 (Mathematical Model) | 降级兜底行为 (Fallback) |
| :--- | :--- | :--- | :--- |
| **IdP 换取 Token** | 开放平台 500/502/503 | `Exponential(base=100ms, max=2s, retries=3)` | 阻断入口并返回 HTTP 502，提示用户重试。 |
| **Sync Hook 调用** | ISV Webhook 响应超时/500 | `Linear(interval=200ms, retries=2)` | 降级处理：**忽略 Webhook**，仅下发网关侧 `cowen_sess_id` 的 302 洗白跳转（降级为第二轨 Pull 模式）。 |
| **获取/刷新 JWKS** | Store 网络断开/读写超时 | `Exponential(base=50ms, max=1s, retries=5)` | 内存中只要有存量 Keys 则无视网络报错；若内存空且重试穷尽，阻塞所有流量，响应 500。 |

## 4. 原子化方法签名 (Atomic Method Signatures)

```rust
// Ingress 引擎入口点
pub async fn evaluate_ingress_pipeline(req: &HttpRequest, cfg: &GatewayConfig) -> GatewayResult;

// JWE 核心加解密组件
pub fn generate_fingerprint(req: &HttpRequest) -> String;
pub fn encrypt_session(payload: &SessionPayload, jwks: &JwksManager) -> Result<String, CryptoError>;
pub fn decrypt_session(cookie_val: &str, jwks: &JwksManager) -> Result<SessionPayload, CryptoError>;

// Sync Hook 适配器
pub async fn invoke_isv_webhook(url: &str, user_ctx: &UserContext) -> Result<Vec<Cookie>, WebhookError>;

// 离散滑动窗口评判
pub fn should_refresh_session(jwe: &SessionPayload, threshold_sec: i64) -> bool;
```

## 5. TDD 验证契约 (TDD Verification Contracts)

1. **[Code 全局拦截]**
   - **Given** URL = `/public/page?code=123` 且命中 Bypass (白名单).
   - **When** 收到请求.
   - **Then** 断言未透传给后端 -> 断言发起了 IdP 换票 -> 断言响应是 302 `/public/page` -> 断言存在 `Set-Cookie: cowen_sess_id`.
2. **[离散续期与双向 Cookie 下发]**
   - **Given** 处于临期区（idle_exp 剩余 < 10min）且 fp 指纹匹配的合法请求.
   - **When** Ingress Operator 执行完毕.
   - **Then** 断言透传头含 `x-org-id` -> 断言网关挂载了拦截回调 -> 断言最终发给浏览器的 HTTP 响应中存在刷新了 `idle_exp` 的 `Set-Cookie: cowen_sess_id`.
3. **[黑白名单降级与 CORS 适配]**
   - **Given** Header `Accept: application/json` 且无会话的未授权请求命中 Require 黑名单.
   - **When** 执行路由解析.
   - **Then** 断言无 302 发生 -> 断言响应 401 并在 Body 中提供 `login_url`.
4. **[指纹防盗刷截断]**
   - **Given** 拥有合法有效期 JWE 载荷的 Cookie，但其签发时的 `IP_HASH` 与当前请求 `IP` 不一致.
   - **When** JWE 解密算子验证.
   - **Then** 断言 `fp` 匹配失败 -> 断言视同未登录，阻断并要求重新授权。
5. **[Sync Hook 降级]**
   - **Given** 开启 Sync Hook，但对应的 webhook_url 模拟 500 宕机.
   - **When** Code 拦截完成，尝试回调 ISV.
   - **Then** 断言在重试 2 次后 -> 断言网关不抛出异常 -> 断言依旧成功下发 302 和网关 Cookie (优雅降级).
