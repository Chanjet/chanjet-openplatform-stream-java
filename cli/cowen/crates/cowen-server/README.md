# Cowen Server

畅捷通 Cowen CLI 的后台运行引擎与 Streaming Gateway 桥接器。

## 🎯 职责 (Responsibility)
- **守护进程管理 (Daemonization)**: 负责 CLI 的后台化运行、PID 追踪及自愈恢复。
- **Streaming 桥接 (Sidecar Bridge)**: 基于 WebSocket 实现云端消息到本地 Webhook 的高性能、低延迟转发。
- **本地安全代理 (Security Proxy)**: 托管本地 HTTP 代理服务，自动注入生产环境鉴权头，实现“零配置”调用。
- **运维观察 (DLQ & Observability)**: 管理死信队列 (DLQ)，确保消息投递的可靠性。

## 🛠️ 核心能力 (Capabilities)
- **StreamingGateway**: 维持与云端的高可靠双向长连接。
- **Forwarder**: 智能消息转发引擎，支持失败重试与 DLQ 自动压入。
- **LocalProxy**: 基于 `axum` 的轻量级安全代理。
- **ServiceManager**: 支持全平台（macOS/Linux/Windows）的自启动服务安装与卸载。

## 📦 外部依赖 (Key Dependencies)
- `axum`: 本地代理服务端框架。
- `tokio-tungstenite`: WebSocket 协议实现。
- `cowen-auth`: 依赖其令牌维护能力。

## ⚠️ 注意事项 (Constraints)
- **并发安全**: 必须处理高并发下的端口竞争与资源泄漏问题。
- **非阻塞设计**: 确保长连接维护、消息转发与代理服务在同一进程内高效并行。
