# 非功能性设计 (NFR Design)

## 1. 安全性实施路径
- **传输安全**：所有外部连接（DB, Redis, API）强制启用 TLS 1.2+。
- **存储加密**：使用 `aes-gcm` 库实现数据入库前的加解密。密钥由 `COWEN_MASTER_KEY` 环境变量注入。
- **签名校验**：Webhook 入站强制核销 `x-chanjet-signature`。

## 2. 可观测性设计
- **日志**：使用 `tracing` + `tracing-appender`。支持 JSON 格式以便于 ELK/Loki 采集。
- **监控**：暴露 `/metrics` 端口（Prometheus 格式），监控 Proxy 并发数、存储连接池状态、Token 刷新延迟。

## 3. 高可用设计
- **分布式锁**：
  - Redis 驱动下使用 `Redlock` 算法实现。
  - SQL 驱动下使用 `SELECT ... FOR UPDATE` 或乐观锁版本号。
- **降级**：若 Cache 挂掉，Auth 模块需能直接回源 DB。

---
*关联 SRS：[非功能性需求](../../srs/sections/02-nfr.md)*
