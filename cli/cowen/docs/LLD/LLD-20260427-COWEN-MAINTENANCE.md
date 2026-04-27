# Cowen CLI 维护与优化 LLD (LLD-20260427-COWEN-MAINTENANCE)

## 1. 物理模型契约 (Physical Model Contract)

### 1.1 状态枚举 (State Enums)
```rust
/// 认证会话状态映射至 Vault 存储
pub enum AuthSessionState {
    Pending,   // 已创建，等待回调 (pending_auth_session)
    Captured,  // 已捕获 Code (captured_auth_code)
    Finalized, // 已换取 Token (oauth2_token_pair)
    None       // 无活跃会话
}

/// 守护进程运行健康状态
pub enum DaemonHealth {
    Running,   // PID 存在且端口响应正常
    Hanging,   // PID 存在但端口无响应 (Windows 更新后常见)
    Stopped,   // PID 存在但进程实际不存在
    None       // 未启动
}
```

### 1.2 I/O 模型
- **TCP 探测**: 发送空的 TCP 连接请求至 `127.0.0.1:<PORT>`。
- **Vault 记录**: `pending_auth_session`, `captured_auth_code` 存储为 JSON 字符串。

---

## 2. 确定性逻辑算子 (Deterministic Logic Operators)

### 2.1 Webhook 回环校验逻辑 (SEC-20260423)
**判定树**:
1. 输入地址字符串 `addr`。
2. 解析为 `SocketAddr`。
3. 判定 `ip()` 是否为 `127.0.0.1` (IPv4) 或 `::1` (IPv6)。
4. 若为 `0.0.0.0` 或其他公网/局域网 IP -> 报错 `SecurityError::IllegalBinding`。
5. 若为回环地址 -> 允许 `TcpListener::bind`。

### 2.2 OAuth2 会话清理流程 (UX-20260423)
**算法步骤**:
1. **前置清理**: 在 `login()` 函数开始时，调用 `session_manager.clear(profile)`。
2. **异常清理**: 
   - 使用 `tokio::select!` 监听认证任务。
   - 在 `Timeout` 分支、`AuthRejected` 分支、`ExchangeError` 分支，统一进入 `cleanup` 逻辑。
3. **清理操作**:
   - `vault.delete("pending_auth_session")`
   - `vault.delete("captured_auth_code")`

### 2.3 守护进程功能性健康检查 (BUG-20260423)
**算法步骤**:
1. **PID 检查**: 读取 PID 文件，使用 `sysinfo` 确认进程是否存在。
2. **端口探测**:
   - 若进程存在，尝试连接 `127.0.0.1:<PROXY_PORT>`。
   - 设置连接超时时间为 1 秒。
3. **判定自愈**:
   - 若连接被拒绝 (ConnectionRefused) 或超时，判定为 `Hanging`。
   - 记录审计日志，强制 `kill` 该 PID，并删除 PID 文件。
   - 调用 `start_daemon` 重新拉起进程。

---

## 3. 健壮性重试矩阵 (Robustness Retry Matrix)

| 场景 | 探测动作 | 重试次数 | 间隔时间 (指数退避) | 最终 Action |
| :--- | :--- | :--- | :--- | :--- |
| 守护进程状态探测 | TCP Connect | 3 | [1s, 2s, 4s] | 判定为 Hanging，执行重启 |
| 令牌换取请求 | HTTP POST | 2 | [2s, 5s] | 记录错误并清理会话 |

---

## 4. 原子化方法签名 (Atomic Method Signatures)

```rust
// src/core/network.rs
pub fn validate_loopback_addr(addr: &SocketAddr) -> Result<(), crate::core::security::SecurityError>;

// src/auth/lifecycle/mod.rs
impl AuthSessionManager {
    pub fn clear(&self, profile: &str) -> Result<()>;
}

// src/cmd/system.rs
async fn is_port_responsive(port: u16) -> bool;
async fn recover_hanging_daemon(profile: &str, pid: u32) -> Result<()>;
```

---

## 5. TDD 验证契约 (TDD Validation Contract)

### 5.1 逻辑分支映射
- **Test Case 1 (Security)**:
  - **Given**: 输入 `0.0.0.0:8080`
  - **When**: 调用 `validate_loopback_addr`
  - **Then**: 必须返回 `Err`
- **Test Case 2 (UX - Cleanup)**:
  - **Given**: 存在 `pending_auth_session`
  - **When**: 模拟 `tokio::time::sleep` 触发超时
  - **Then**: Vault 中对应的 Key 必须被删除
- **Test Case 3 (Bugfix - Windows)**:
  - **Given**: 模拟 PID 存在但无监听端口的情况
  - **When**: 执行 `ensure_daemon_running`
  - **Then**: 旧进程被清理，新进程被成功拉起并监听端口

---
**小结**: 该 LLD 为 PRD 中的 4 项改进提供了执行级的实现细节，确保逻辑确定性与系统健壮性。
