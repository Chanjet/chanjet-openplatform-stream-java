# Cowen Common

畅捷通 Cowen CLI 的基础基础设施 crate。本组件定义了全工程共享的元数据、配置模型、工具类及核心 Trait 契约。

## 🎯 职责 (Responsibility)
- **统一模型 (Unified Models)**: 定义 `Token`, `Ticket`, `AuditEntry`, `DlqMessage` 等基础业务对象。
- **配置治理 (Config Governance)**: 负责 `app.yaml` 与 Profile 配置的物理加载、反序列化以及优先级管理（环境变量覆盖）。
- **SPI 契约 (SPI Contracts)**: 定义 `Vault`, `Store`, `TicketDomain`, `TokenDomain` 等核心接口，实现逻辑解耦。
- **安全基座 (Security Foundation)**: 提供 `obfs!` 字符串混淆、AES/GCM 加解密、机器指纹生成等安全工具。

## 🛠️ 核心能力 (Capabilities)
- **ConfigManager**: 支持多 Profile 隔离的配置生命周期管理。
- **StatusCollector**: 标准化的诊断采集框架。
- **Network**: 基于 `reqwest` 的高度定制化 HTTP 客户端，支持代理与重试逻辑。
- **EventBus**: 进程内广播事件总线。

## 📦 外部依赖 (Key Dependencies)
- `serde`: 序列化框架。
- `tokio`: 异步运行时及同步原语。
- `anyhow`: 统一错误处理。
- `chrono`: 时间与日期处理。

## ⚠️ 注意事项 (Constraints)
- **禁止向上依赖**: 本 crate 是 Workspace 的根部，严禁依赖 `cowen-auth`, `cowen-store` 或 `cowen-server`。
- **通用性优先**: 仅存放全工程通用的逻辑，特定业务逻辑应下沉至对应领域 crate。
