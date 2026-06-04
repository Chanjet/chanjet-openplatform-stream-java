# Cowen CLI 自动化测试用例说明文档 (E2E Test Suites)

本文档详细说明了 Cowen CLI 各个端到端 (E2E) 测试用例的设计场景、测试目标及功能规范。

---

## 1. 基础初始化与配置 (Basic Init & Config)

### [Case 01] 自建应用模式初始化 (`case_01_self_built.sh`)
*   **测试场景**: 在 `self-built` 模式下执行 `init`。
*   **测试目标**: 验证基本的 AppKey/AppSecret 初始化流程。
*   **功能规范**: 成功生成配置文件，并在本地 SQLite 建立初始元数据。

### [Case 02] 商店应用模式初始化 (`case_02_store_app.sh`)
*   **测试场景**: 在 `store-app` (Sidecar) 模式下执行 `init`。
*   **测试目标**: 验证无需 AppSecret 的 Sidecar 初始化路径。
*   **功能规范**: 验证凭证获取逻辑是否正确切换到应用商店授权链路。

### [Case 37] 初始化失败清理 (`case_37_init_cleanup.sh`)
*   **测试场景**: 在 `init` 过程中发生错误或手动取消（Ctrl+C）。
*   **测试目标**: 验证中间状态的清理机制。
*   **功能规范**: 确保在初始化失败时，不会在磁盘上留下破损的 Profile 文件或数据库条目。

### [Case 38] 初始化去重 (`case_38_init_deduplication.sh`)
*   **测试场景**: 使用相同的 AppKey 和模式重复初始化不同的 Profile。
*   **测试目标**: 验证系统对重复实例的识别能力。
*   **功能规范**: 禁止创建指向同一个云端实例的多个 Profile，防止状态冲突。

---

## 2. 身份认证与令牌管理 (Auth & Token Management)

### [Case 03] OAuth2 全生命周期验证 (`case_03_oauth2.sh`)
*   **测试场景**: 完整的 OAuth2 授权码 (PKCE) 流程。
*   **测试目标**: 验证阻塞式初始化、回调监听、令牌交换、刷新全过程。
*   **功能规范**: 模拟浏览器回调，验证 Token 能自动归档并触发守护进程。

### [Case 07] 令牌获取与归档 (`case_07_token_lifecycle.sh`)
*   **测试场景**: 执行 `auth token` 命令。
*   **测试目标**: 验证令牌的获取、持久化存储及后续直接读取逻辑。
*   **功能规范**: 确保令牌被加密存储，并在有效期内优先从本地读取。

### [Case 20] OAuth2 自动刷新机制 (`case_20_oauth2_refresh.sh`)
*   **测试场景**: 模拟 OAuth2 访问令牌过期。
*   **测试目标**: 验证守护进程能否自动通过 Refresh Token 获取新令牌。
*   **功能规范**: 在 Token 接近过期前，后台自动触发异步刷新逻辑。

### [Case 19] 企业令牌 (AppTicket) 自动补发 (`case_19_ticket_auto_resend.sh`)
*   **测试场景**: 在 `store-app` 模式下，本地缺失 AppTicket 或 AppTicket 失效。
*   **测试目标**: 验证自动触发云端补发流程。
*   **功能规范**: 触发 `/auth/appTicket/resend`，并在收到 Webhook 后自动更新本地存储。

### [Case 41] 登出与重新登录流程 (`case_41_auth_logout_login_flow.sh`)
*   **测试场景**: 执行 `auth logout` 后再次执行 `auth login`。
*   **测试目标**: 验证令牌清除的彻底性和重新登录的无缝性。
*   **功能规范**: Logout 必须清除所有敏感令牌；Login 在无令牌时必须自动拉起授权流。

---

## 3. 代理、消息转发与独占性 (Proxy & Webhook)

### [Case 05] Proxy API 拦截与注入 (`case_05_proxy_interception.sh`)
*   **测试场景**: 开发者通过 Sidecar 代理端口调用 OpenAPI。
*   **测试目标**: 验证身份验证头 (Authorization) 的自动注入。
*   **功能规范**: 代理必须自动识别租户身份，并无感注入最新的 Access Token。

### [Case 06] Webhook 消息转发 (`case_06_webhook_forwarding.sh`)
*   **测试场景**: 云端推送 Webhook 消息。
*   **测试目标**: 验证消息从长连接接收到转发至本地 `webhook-target` 的全链路。
*   **功能规范**: 验证转发时的签名重构、幂等性处理以及 HTTP 状态码处理。

### [Case 33] 独占连接模式 (`case_33_exclusive_connection.sh`)
*   **测试场景**: 多个相同 AppKey 的实例开启 `exclusive` 模式。
*   **测试目标**: 验证“后浪推前浪”的剔除机制。
*   **功能规范**: 新连接建立时，云端必须主动断开旧连接，确保同一时刻只有一个活跃长连接。

### [Case 21] OpenAPI 白名单过滤 (`case_21_openapi_whitelist.sh`)
*   **测试场景**: 调用非开放范围内的 API。
*   **测试目标**: 验证本地安全过滤机制。
*   **功能规范**: 代理层必须能拦截未授权的域名访问。

---

## 4. 分布式与共享存储 (Distributed & Multi-Node)

### [Case 13] 负载均衡分发验证 (`case_13_distributed_lb.sh`)
*   **测试场景**: 多个 Cowen 节点连接同一个 AppKey，云端下发负载均衡消息。
*   **测试目标**: 验证消息能被均匀/随机分布到各个活跃节点。
*   **功能规范**: 确保消息不重不漏，且多个节点能共存。

### [Case 14] 共享 SQLite 存储同步 (`case_14_shared_storage.sh`)
*   **测试场景**: 两个节点挂载同一个 NFS 路径下的 SQLite。
*   **测试目标**: 验证文件级锁处理和状态实时刷新。
*   **功能规范**: 节点 A 刷新的 Token，节点 B 必须能立即感知并使用。

### [Case 17] Redis 共享存储验证 (`case_17_redis_shared_storage.sh`)
*   **测试场景**: 使用 Redis 作为外部存储引擎。
*   **测试目标**: 验证分布式缓存的读取与失效逻辑。
*   **功能规范**: 确保跨节点 Token 同步的毫秒级一致性。

### [Case 31/32] MySQL/PostgreSQL 共享存储 (`case_31_mysql_shared_storage.sh`)
*   **测试场景**: 使用标准 RDBMS 作为持久化后端。
*   **测试目标**: 验证大规模集群下的连接池管理与事务一致性。
*   **功能规范**: 确保 Token、Profile 等核心元数据在关系型数据库中的正确读写。

### [Case 25] 消息处理幂等性 (Idempotency) (`case_25_cluster_idempotency.sh`)
*   **测试场景**: 多个节点同时收到同一个 `msgId`。
*   **测试目标**: 验证集群级别的去重。
*   **功能规范**: 即使多个节点同时接到消息，由于共享存储的锁机制，只能有一个节点成功执行转发任务。

---

## 5. 健壮性与异常恢复 (Resilience & Recovery)

### [Case 11] 断线重连韧性 (`case_11_reconnect_resilience.sh`)
*   **测试场景**: 模拟网络抖动或服务端滚动重启。
*   **测试目标**: 验证退避重试 (Exponential Backoff) 算法。
*   **功能规范**: 确保在网络恢复后，长连接能自动建立并恢复消息监听。

### [Case 12/34] 守护进程自动恢复 (`case_12_daemon_recovery.sh`)
*   **测试场景**: 守护进程被外部 `kill -9` 或因异常崩溃。
*   **测试目标**: 验证“按需启动”的自我修复能力。
*   **功能规范**: 当用户执行任意命令（如 `status` 或 `token`）时，系统检测到守护进程缺失应能自动将其拉起。

### [Case 09] 死信队列 (DLQ) 重试 (`case_09_dlq_retries.sh`)
*   **测试场景**: 本地业务接收端 (Webhook Target) 响应 500 或超时。
*   **测试目标**: 验证本地重试队列逻辑。
*   **功能规范**: 消息必须进入 DLQ，并在之后的时间窗口内按策略自动重试。

### [Case 22] DLQ 手动重试与清理 (`case_22_dlq_manual_retry.sh`)
*   **测试场景**: 通过 `dlq retry` 或 `dlq clear` 命令操作系统。
*   **测试目标**: 验证人工干预死信消息的能力。
*   **功能规范**: 确保堆积的消息能被手动重新推入处理流。

---

## 6. 运维与周边功能 (Operations & Utilities)

### [Case 04/16] 数据迁移验证 (`case_04_migration.sh`)
*   **测试场景**: 将数据从 Local SQLite 迁移到 Redis/SQL 存储。
*   **测试目标**: 验证 `store migrate` 命令的完整性。
*   **功能规范**: 迁移后原始数据保持完整，新存储能无缝接管业务。

### [Case 10/39] Profile 管理与更名 (`case_10_profile_management.sh`)
*   **测试场景**: 执行 `profile rename` / `profile use`。
*   **测试目标**: 验证配置文件的物理重命名和内存缓存更新。
*   **功能规范**: 确保更名后所有关联的数据库条目和守护进程状态能正确同步。

### [Case 40] 动态日志级别调整 (`case_40_log_level_dynamic.sh`)
*   **测试场景**: 运行时修改 `log.level` 配置。
*   **测试目标**: 验证无需重启守护进程即可生效。
*   **功能规范**: 验证不同日志级别（Debug, Info, Warn, Error）的过滤效果。

### [Case 23] Shell 补全脚本 (`case_23_completion.sh`)
*   **测试场景**: 执行 `completion bash/zsh`。
*   **测试目标**: 验证自动补全脚本的生成与安装。
*   **功能规范**: 验证生成的脚本在对应 Shell 中能正确解析命令树。

---
**提示**：所有测试用例均支持在本地环境通过 `make test-macos` 或在 Linux 容器内通过 `make test-linux` 运行。
