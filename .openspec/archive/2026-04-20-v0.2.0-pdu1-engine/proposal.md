# Proposal: Cowen CLI v0.2.0 PDU 1 - OAuth2 Protocol Engine

## Why (驱动背景)
OAuth2 (PKCE) 模式的核心在于安全的令牌交换与维持。由于 Refresh Token 具有单次使用失效的特性，且 CLI 与 Daemon 可能在多进程环境下并发运行，因此需要实现精确的 PKCE 协议引擎、具备“二次检查”逻辑的并发文件锁机制，以及令牌自动轮换逻辑。

## What Changes (变更内容)
- **协议实现**：
  - 实现 `PKCE` 辅助类，生成 64 位 Verifier 及 S256 Challenge。
  - 实现 `OAuth2Provider`：支持 `exchange_code`、`refresh` 两种授权模式。
- **并发保护**：
  - 集成 `fs2` 实现 Profile 级文件锁。
  - 实现“获取锁 -> 重新读取 -> 必要时网络请求”的 Double-Check 逻辑。
- **令牌维护**：
  - 实现 Access Token 自动刷新及 Refresh Token 物理轮换存储。
- **错误处理**：
  - 映射平台错误码（4007, 4029 等）为 CLI 业务异常。

## Impact (影响范围)
- **规范**：新增 `OAuth2Provider` 行为规范。
- **代码**：主要集中在 `src/auth/provider/oauth2.rs`，且不破坏 `AuthProvider` 接口。
- **存储**：Vault 中新增 `oauth2_token_pair` 序列化存储。

## Verification Plan (验证计划)
- **TDD**：针对 PKCE 生成、Token 换票、并发锁竞争编写单元测试与 Mock 集成测试。
- **Mock**：模拟 4007/4029 场景验证自愈逻辑。
