# Spec: System Integration Standards (SYKFPT-1061-5.3)

## 1. 运行环境要求
- **Runtime**: OpenJDK 21+ (必需)。
- **GC**: 建议使用 G1 或 ZGC，配合虚拟线程。

## 2. 配置项规范 (application.yml)
| 配置项 | 说明 | 默认值 |
| :--- | :--- | :--- |
| `connector.node-id` | 当前节点的物理标识 | `127.0.0.1:8080` |
| `services.auth.id` | 鉴权服务的 Nacos ServiceId | `cjt-auth-service` |
| `spring.threads.virtual.enabled` | 是否开启虚拟线程 | `true` |

## 3. 部署规范
- **端口**: 默认 8080 (Webhook + WebSocket 共用)。
- **健康检查**: 暴露 `/actuator/health` 路径。

## 4. 安全规范
- 禁止在 `application.yml` 中配置明文 AppSecret。
- 网关与 Core 服务之间的内部通信必须基于 ServiceId 且受内部令牌保护。
