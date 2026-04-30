# 产品需求文档 - 业务建模与功能流转 (PRD Core)

## 业务背景
Cowen v0.3.0 的核心目标是完成从“桌面工具”向“分布式生产边车”的进化。通过引入可插拔存储后端，解决单机部署的物理限制，支持 ISV 应用在混合云或多实例环境下的高可用运行。

## 用户场景与交互链路 (User Journey)
1. **开发者本地调试 (Legacy Mode)**: 用户下载 Cowen，使用 `init` 初始化，默认使用本地文件存储，行为与 v0.2.x 保持一致。
2. **生产环境集群部署 (Distributed Mode)**:
   - 运维通过环境变量或配置文件指定存储类型（如 `mysql`）及连接串。
   - 多个 Cowen 实例连接到同一个 MySQL，实现配置、Token 和 Webhook 状态的实时共享。
   - 实现 Sidecar 的水平扩展。
3. **高性能场景 (Hybrid Mode)**:
   - 运维配置 Redis 作为 Cache，MySQL 作为持久化后端。
   - Token 刷新和查询优先命中 Redis，减少数据库压力，降低 Proxy 转发延迟。

---
*溯源参考：[Cowen v0.2.x 快照](../../references/cowen-v02-snapshot.md#cowen-v02-snapshot) (Verified)*
